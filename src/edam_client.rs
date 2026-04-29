use crate::config::Config;
use crate::editor;
use crate::errors::{ReeknoteError, Result};
use crate::geeknote::{EvernoteClient, ParsedNoteInput, ReminderValue};
use crate::models::{
    Accounting, LinkedNotebook, Note, NoteAttributes, Notebook, Resource, ResourceData,
    SearchResult, Tag, UserInfo,
};
use evernote::note_store::{
    NoteFilter, NoteStoreSyncClient, NotesMetadataResultSpec, TNoteStoreSyncClient,
};
use evernote::types as edam_types;
use evernote::user_store::{
    EDAM_VERSION_MAJOR, EDAM_VERSION_MINOR, TUserStoreSyncClient, UserStoreSyncClient,
};
use std::collections::VecDeque;
use std::fs;
use std::io::{self, Read, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};
use thrift::protocol::{TBinaryInputProtocol, TBinaryOutputProtocol};
use thrift::transport::{TBufferedReadTransport, TBufferedWriteTransport};

#[derive(Clone, Debug)]
pub struct EdamClient {
    auth_token: String,
    user_store_url: String,
    note_store_url: Option<String>,
}

impl EdamClient {
    pub fn new(auth_token: impl Into<String>, config: &Config) -> Self {
        Self {
            auth_token: auth_token.into(),
            user_store_url: config.user_store_uri.clone(),
            note_store_url: None,
        }
    }

    fn user_store(&self) -> UserStore {
        let (read, write) = http_halves(&self.user_store_url);
        UserStoreSyncClient::new(input_protocol(read), output_protocol(write))
    }

    fn note_store(&mut self) -> Result<NoteStore> {
        let url = if let Some(url) = &self.note_store_url {
            url.clone()
        } else {
            let mut user_store = self.user_store();
            let urls = user_store
                .get_user_urls(self.auth_token.clone())
                .map_err(map_thrift_error)?;
            let url = urls.note_store_url.ok_or_else(|| {
                ReeknoteError::External("Evernote did not return a NoteStore URL".to_string())
            })?;
            self.note_store_url = Some(url.clone());
            url
        };
        Ok(note_store_for_url(&url))
    }

    pub fn check_version(&self) -> Result<bool> {
        let mut user_store = self.user_store();
        user_store
            .check_version(
                "Geeknote Rust/0.1".to_string(),
                EDAM_VERSION_MAJOR,
                EDAM_VERSION_MINOR,
            )
            .map_err(map_thrift_error)
    }

    pub fn download_linked_notebook_notes(
        &mut self,
        linked_notebook: &LinkedNotebook,
        count: usize,
        with_resources_data: bool,
    ) -> Result<Vec<Note>> {
        if linked_notebook.note_store_url.is_empty() {
            return Err(ReeknoteError::InvalidInput(format!(
                "linked notebook has no NoteStore URL: {}",
                linked_notebook.share_name
            )));
        }
        if linked_notebook.share_key.is_empty() {
            return Err(ReeknoteError::InvalidInput(format!(
                "linked notebook has no share key/global id: {}",
                linked_notebook.share_name
            )));
        }

        let mut note_store = note_store_for_url(&linked_notebook.note_store_url);
        let auth = note_store
            .authenticate_to_shared_notebook(
                linked_notebook.share_key.clone(),
                self.auth_token.clone(),
            )
            .map_err(map_thrift_error)?;
        let shared_token = auth.authentication_token;
        let shared_notebook = note_store
            .get_shared_notebook_by_auth(shared_token.clone())
            .map_err(map_thrift_error)?;
        let notebook_guid = shared_notebook.notebook_guid.ok_or_else(|| {
            ReeknoteError::External(format!(
                "Evernote did not return a notebook GUID for linked notebook {}",
                linked_notebook.share_name
            ))
        })?;
        let filter = NoteFilter::new(
            Some(edam_types::NoteSortOrder::UPDATED.0),
            None,
            None::<String>,
            Some(notebook_guid),
            None::<Vec<String>>,
            None::<String>,
            None,
            None::<String>,
            None,
            None::<String>,
            None::<String>,
            None::<Vec<u8>>,
            None,
        );
        let result = note_store
            .find_notes_metadata(
                shared_token.clone(),
                filter,
                0,
                count as i32,
                metadata_result_spec(),
            )
            .map_err(map_thrift_error)?;
        let mut notes = Vec::new();
        for metadata in result.notes {
            let mut note = note_from_edam(
                note_store
                    .get_note(
                        shared_token.clone(),
                        metadata.guid,
                        true,
                        with_resources_data,
                        false,
                        false,
                    )
                    .map_err(map_thrift_error)?,
            );
            note.notebook_name = Some(linked_notebook.share_name.clone());
            notes.push(note);
        }
        Ok(notes)
    }
}

type UserStore = UserStoreSyncClient<
    TBinaryInputProtocol<TBufferedReadTransport<HttpReadHalf>>,
    TBinaryOutputProtocol<TBufferedWriteTransport<HttpWriteHalf>>,
>;

type NoteStore = NoteStoreSyncClient<
    TBinaryInputProtocol<TBufferedReadTransport<HttpReadHalf>>,
    TBinaryOutputProtocol<TBufferedWriteTransport<HttpWriteHalf>>,
>;

fn note_store_for_url(url: &str) -> NoteStore {
    let (read, write) = http_halves(url);
    NoteStoreSyncClient::new(input_protocol(read), output_protocol(write))
}

impl EvernoteClient for EdamClient {
    fn get_user_info(&mut self) -> Result<UserInfo> {
        let mut user_store = self.user_store();
        let user = user_store
            .get_user(self.auth_token.clone())
            .map_err(map_thrift_error)?;
        Ok(user_from_edam(user))
    }

    fn get_note(&mut self, guid: &str) -> Result<Note> {
        self.get_note_inner(guid, false)
    }

    fn get_note_with_resources(&mut self, guid: &str) -> Result<Note> {
        self.get_note_inner(guid, true)
    }

    fn get_note_content(&mut self, guid: &str) -> Result<String> {
        let token = self.auth_token.clone();
        let mut note_store = self.note_store()?;
        note_store
            .get_note_content(token, guid.to_string())
            .map_err(map_thrift_error)
    }

    fn find_notes(
        &mut self,
        query: &str,
        count: usize,
        deleted_only: bool,
    ) -> Result<SearchResult> {
        let token = self.auth_token.clone();
        let mut note_store = self.note_store()?;
        let filter = NoteFilter::new(
            Some(edam_types::NoteSortOrder::UPDATED.0),
            None,
            if query.is_empty() {
                None
            } else {
                Some(query.to_string())
            },
            None,
            None,
            None,
            if deleted_only { Some(true) } else { None },
            None,
            None,
            None,
            None,
            None,
            None,
        );
        let result = note_store
            .find_notes_metadata(token, filter, 0, count as i32, metadata_result_spec())
            .map_err(map_thrift_error)?;
        let notes = result
            .notes
            .into_iter()
            .map(note_metadata_from_edam)
            .collect();
        Ok(SearchResult {
            total_notes: result.total_notes as usize,
            notes,
        })
    }

    fn create_note(&mut self, input: ParsedNoteInput) -> Result<Note> {
        let token = self.auth_token.clone();
        let mut note_store = self.note_store()?;
        let note = note_to_edam(input, None)?;
        let note = note_store
            .create_note(token, note)
            .map_err(map_thrift_error)?;
        Ok(note_from_edam(note))
    }

    fn update_note(&mut self, guid: &str, input: ParsedNoteInput) -> Result<()> {
        let token = self.auth_token.clone();
        let mut note_store = self.note_store()?;
        let note = note_to_edam(input, Some(guid.to_string()))?;
        note_store
            .update_note(token, note)
            .map_err(map_thrift_error)?;
        Ok(())
    }

    fn remove_note(&mut self, guid: &str) -> Result<()> {
        let token = self.auth_token.clone();
        let mut note_store = self.note_store()?;
        note_store
            .delete_note(token, guid.to_string())
            .map_err(map_thrift_error)?;
        Ok(())
    }

    fn find_notebooks(&mut self) -> Result<Vec<Notebook>> {
        let token = self.auth_token.clone();
        let mut note_store = self.note_store()?;
        Ok(note_store
            .list_notebooks(token)
            .map_err(map_thrift_error)?
            .into_iter()
            .map(notebook_from_edam)
            .collect())
    }

    fn find_linked_notebooks(&mut self) -> Result<Vec<LinkedNotebook>> {
        let token = self.auth_token.clone();
        let mut note_store = self.note_store()?;
        Ok(note_store
            .list_linked_notebooks(token)
            .map_err(map_thrift_error)?
            .into_iter()
            .map(linked_notebook_from_edam)
            .collect())
    }

    fn create_notebook(&mut self, name: &str, stack: Option<&str>) -> Result<Notebook> {
        let token = self.auth_token.clone();
        let mut note_store = self.note_store()?;
        let notebook = edam_types::Notebook::new(
            None,
            Some(name.to_string()),
            None,
            None,
            None,
            None,
            None,
            None,
            stack.map(ToOwned::to_owned),
            None,
            None,
            None,
            None,
            None,
            None,
        );
        let notebook = note_store
            .create_notebook(token, notebook)
            .map_err(map_thrift_error)?;
        Ok(notebook_from_edam(notebook))
    }

    fn update_notebook(&mut self, guid: &str, name: &str) -> Result<()> {
        let token = self.auth_token.clone();
        let mut note_store = self.note_store()?;
        let notebook = edam_types::Notebook::new(
            Some(guid.to_string()),
            Some(name.to_string()),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );
        note_store
            .update_notebook(token, notebook)
            .map_err(map_thrift_error)?;
        Ok(())
    }

    fn remove_notebook(&mut self, guid: &str) -> Result<()> {
        let token = self.auth_token.clone();
        let mut note_store = self.note_store()?;
        note_store
            .expunge_notebook(token, guid.to_string())
            .map_err(map_thrift_error)?;
        Ok(())
    }

    fn find_tags(&mut self) -> Result<Vec<Tag>> {
        let token = self.auth_token.clone();
        let mut note_store = self.note_store()?;
        Ok(note_store
            .list_tags(token)
            .map_err(map_thrift_error)?
            .into_iter()
            .map(tag_from_edam)
            .collect())
    }

    fn create_tag(&mut self, name: &str) -> Result<Tag> {
        let token = self.auth_token.clone();
        let mut note_store = self.note_store()?;
        let tag = edam_types::Tag::new(None, Some(name.to_string()), None, None);
        let tag = note_store
            .create_tag(token, tag)
            .map_err(map_thrift_error)?;
        Ok(tag_from_edam(tag))
    }

    fn update_tag(&mut self, guid: &str, name: &str) -> Result<()> {
        let token = self.auth_token.clone();
        let mut note_store = self.note_store()?;
        let tag = edam_types::Tag::new(Some(guid.to_string()), Some(name.to_string()), None, None);
        note_store
            .update_tag(token, tag)
            .map_err(map_thrift_error)?;
        Ok(())
    }

    fn remove_tag(&mut self, guid: &str) -> Result<()> {
        let token = self.auth_token.clone();
        let mut note_store = self.note_store()?;
        note_store
            .expunge_tag(token, guid.to_string())
            .map_err(map_thrift_error)?;
        Ok(())
    }
}

impl EdamClient {
    fn get_note_inner(&mut self, guid: &str, with_resources_data: bool) -> Result<Note> {
        let token = self.auth_token.clone();
        let mut note_store = self.note_store()?;
        let note = note_store
            .get_note(
                token,
                guid.to_string(),
                true,
                with_resources_data,
                false,
                false,
            )
            .map_err(map_thrift_error)?;
        Ok(note_from_edam(note))
    }
}

#[derive(Clone)]
struct HttpThriftState {
    url: String,
    request: Vec<u8>,
    response: VecDeque<u8>,
}

#[derive(Clone)]
pub struct HttpReadHalf {
    state: Arc<Mutex<HttpThriftState>>,
}

#[derive(Clone)]
pub struct HttpWriteHalf {
    state: Arc<Mutex<HttpThriftState>>,
}

fn input_protocol(
    read: HttpReadHalf,
) -> TBinaryInputProtocol<TBufferedReadTransport<HttpReadHalf>> {
    TBinaryInputProtocol::new(TBufferedReadTransport::new(read), true)
}

fn output_protocol(
    write: HttpWriteHalf,
) -> TBinaryOutputProtocol<TBufferedWriteTransport<HttpWriteHalf>> {
    TBinaryOutputProtocol::new(TBufferedWriteTransport::new(write), true)
}

fn http_halves(url: &str) -> (HttpReadHalf, HttpWriteHalf) {
    let state = Arc::new(Mutex::new(HttpThriftState {
        url: url.to_string(),
        request: Vec::new(),
        response: VecDeque::new(),
    }));
    (
        HttpReadHalf {
            state: Arc::clone(&state),
        },
        HttpWriteHalf { state },
    )
}

impl Read for HttpReadHalf {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| io::Error::other("HTTP Thrift state lock poisoned"))?;
        let mut written = 0;
        while written < buffer.len() {
            let Some(byte) = state.response.pop_front() else {
                break;
            };
            buffer[written] = byte;
            written += 1;
        }
        Ok(written)
    }
}

impl Write for HttpWriteHalf {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| io::Error::other("HTTP Thrift state lock poisoned"))?;
        state.request.extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        let (url, body) = {
            let mut state = self
                .state
                .lock()
                .map_err(|_| io::Error::other("HTTP Thrift state lock poisoned"))?;
            let body = std::mem::take(&mut state.request);
            (state.url.clone(), body)
        };

        let response = reqwest::blocking::Client::new()
            .post(&url)
            .header("Content-Type", "application/x-thrift")
            .header("Accept", "application/x-thrift")
            .body(body)
            .send()
            .and_then(|response| response.error_for_status())
            .map_err(|error| io::Error::other(error.to_string()))?
            .bytes()
            .map_err(|error| io::Error::other(error.to_string()))?;

        let mut state = self
            .state
            .lock()
            .map_err(|_| io::Error::other("HTTP Thrift state lock poisoned"))?;
        state.response = response.iter().copied().collect();
        Ok(())
    }
}

fn map_thrift_error(error: thrift::Error) -> ReeknoteError {
    ReeknoteError::External(format!("Evernote API error: {error}"))
}

fn metadata_result_spec() -> NotesMetadataResultSpec {
    NotesMetadataResultSpec::new(
        Some(true),
        Some(true),
        Some(true),
        Some(true),
        None,
        None,
        Some(true),
        Some(true),
        Some(true),
        Some(true),
        Some(true),
    )
}

fn user_from_edam(user: edam_types::User) -> UserInfo {
    let accounting = user.accounting.unwrap_or_default();
    UserInfo {
        username: user.username.unwrap_or_default(),
        name: user.name.unwrap_or_default(),
        email: user.email.unwrap_or_default(),
        id: user.id.unwrap_or_default() as i64,
        shard_id: user.shard_id.unwrap_or_default(),
        accounting: Accounting {
            upload_limit: 0,
            upload_limit_end: accounting.upload_limit_end,
        },
        timezone: user.timezone,
    }
}

fn note_metadata_from_edam(note: evernote::note_store::NoteMetadata) -> Note {
    Note {
        guid: note.guid,
        title: note.title.unwrap_or_default(),
        content: String::new(),
        created: note.created,
        updated: note.updated,
        notebook_guid: note.notebook_guid,
        tag_guids: note.tag_guids.unwrap_or_default(),
        attributes: note
            .attributes
            .map(attributes_from_edam)
            .unwrap_or_default(),
        content_length: note.content_length.map(|value| value as usize),
        largest_resource_mime: note.largest_resource_mime,
        largest_resource_size: note.largest_resource_size.map(|value| value as usize),
        ..Note::default()
    }
}

fn note_from_edam(note: edam_types::Note) -> Note {
    Note {
        guid: note.guid.unwrap_or_default(),
        title: note.title.unwrap_or_default(),
        content: note.content.unwrap_or_default(),
        created: note.created,
        updated: note.updated,
        notebook_guid: note.notebook_guid,
        tag_guids: note.tag_guids.unwrap_or_default(),
        tag_names: note.tag_names.unwrap_or_default(),
        attributes: note
            .attributes
            .map(attributes_from_edam)
            .unwrap_or_default(),
        resources: note
            .resources
            .unwrap_or_default()
            .into_iter()
            .map(resource_from_edam)
            .collect(),
        content_length: note.content_length.map(|value| value as usize),
        ..Note::default()
    }
}

fn attributes_from_edam(attributes: edam_types::NoteAttributes) -> NoteAttributes {
    NoteAttributes {
        source_url: attributes.source_u_r_l,
        reminder_order: attributes.reminder_order,
        reminder_time: attributes.reminder_time,
        reminder_done_time: attributes.reminder_done_time,
    }
}

fn notebook_from_edam(notebook: edam_types::Notebook) -> Notebook {
    Notebook {
        guid: notebook.guid.unwrap_or_default(),
        name: notebook.name.unwrap_or_default(),
        stack: notebook.stack,
    }
}

fn linked_notebook_from_edam(notebook: edam_types::LinkedNotebook) -> LinkedNotebook {
    LinkedNotebook {
        guid: notebook.guid.unwrap_or_default(),
        share_name: notebook.share_name.unwrap_or_default(),
        share_key: notebook.shared_notebook_global_id.unwrap_or_default(),
        shard_id: notebook.shard_id.unwrap_or_default(),
        note_store_url: notebook.note_store_url.unwrap_or_default(),
    }
}

fn tag_from_edam(tag: edam_types::Tag) -> Tag {
    Tag {
        guid: tag.guid.unwrap_or_default(),
        name: tag.name.unwrap_or_default(),
    }
}

fn resource_from_edam(resource: edam_types::Resource) -> Resource {
    let data = resource
        .data
        .unwrap_or_else(|| edam_types::Data::new(None::<Vec<u8>>, Some(0), Some(Vec::new())));
    let filename = resource
        .attributes
        .and_then(|attributes| attributes.file_name)
        .unwrap_or_default();
    Resource {
        mime: resource.mime,
        filename,
        data: ResourceData {
            body_hash: data
                .body_hash
                .as_ref()
                .map(|hash| hex_encode(hash))
                .unwrap_or_default(),
            body: data.body.unwrap_or_default(),
            size: data.size.unwrap_or_default() as usize,
        },
    }
}

fn note_to_edam(input: ParsedNoteInput, guid: Option<String>) -> Result<edam_types::Note> {
    let mut attributes = edam_types::NoteAttributes::default();
    if let Some(url) = input.url {
        attributes.source_u_r_l = Some(url);
    }
    match input.reminder {
        Some(ReminderValue::Timestamp(timestamp)) => {
            attributes.reminder_order = Some(now_millis());
            attributes.reminder_time = Some(timestamp);
        }
        Some(ReminderValue::None) => {
            attributes.reminder_order = Some(now_millis());
        }
        Some(ReminderValue::Done) => {
            let now = now_millis();
            attributes.reminder_order = Some(now);
            attributes.reminder_done_time = Some(now);
        }
        Some(ReminderValue::Delete) | None => {}
    }

    let resources = input
        .resources
        .iter()
        .map(|path| resource_to_edam(path))
        .collect::<Result<Vec<_>>>()?;

    let mut content = input.content.unwrap_or_else(|| editor::text_to_enml(" "));
    if !resources.is_empty() {
        let resource_nodes = resources
            .iter()
            .filter_map(|resource| {
                Some(format!(
                    "<en-media type=\"{}\" hash=\"{}\" />",
                    resource.mime.as_ref()?,
                    hex_encode(resource.data.as_ref()?.body_hash.as_ref()?)
                ))
            })
            .collect::<String>();
        content = content.replace("</en-note>", &format!("{resource_nodes}</en-note>"));
    }

    Ok(edam_types::Note::new(
        guid,
        input.title,
        Some(content),
        None,
        None,
        input.created,
        None,
        None,
        None,
        None,
        input.notebook,
        None,
        if resources.is_empty() {
            None
        } else {
            Some(resources)
        },
        Some(attributes),
        if input.tags.is_empty() {
            None
        } else {
            Some(input.tags)
        },
        None,
        None,
        None,
    ))
}

fn resource_to_edam(path: &str) -> Result<edam_types::Resource> {
    let body = fs::read(path)?;
    let digest = md5::compute(&body);
    let hash = digest.0.to_vec();
    let mime = mime_guess::from_path(path)
        .first()
        .map(|mime| mime.essence_str().to_string());
    let filename = Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(path)
        .to_string();
    let data = edam_types::Data::new(Some(hash), Some(body.len() as i32), Some(body));
    let attributes = edam_types::ResourceAttributes::new(
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        Some(filename),
        None,
        None,
    );
    Ok(edam_types::Resource::new(
        None,
        None,
        Some(data),
        mime,
        None,
        None,
        None,
        None,
        None,
        Some(attributes),
        None,
        None,
    ))
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_edam_note_metadata() {
        let metadata = evernote::note_store::NoteMetadata::new(
            "guid".to_string(),
            Some("title".to_string()),
            Some(12),
            Some(1),
            Some(2),
            None,
            None,
            Some("nb".to_string()),
            Some(vec!["tag".to_string()]),
            None,
            None,
            None,
        );
        let note = note_metadata_from_edam(metadata);
        assert_eq!(note.guid, "guid");
        assert_eq!(note.title, "title");
        assert_eq!(note.notebook_guid.as_deref(), Some("nb"));
        assert_eq!(note.tag_guids, vec!["tag"]);
    }

    #[test]
    fn creates_edam_note_from_input() {
        let input = ParsedNoteInput {
            title: Some("title".to_string()),
            content: Some(editor::text_to_enml("body")),
            tags: vec!["tag".to_string()],
            ..ParsedNoteInput::default()
        };
        let note = note_to_edam(input, None).unwrap();
        assert_eq!(note.title.as_deref(), Some("title"));
        assert!(note.content.unwrap().contains("body"));
        assert_eq!(note.tag_names.unwrap(), vec!["tag"]);
    }

    #[test]
    fn maps_edam_linked_notebook() {
        let linked = edam_types::LinkedNotebook::new(
            Some("Shared".to_string()),
            Some("owner".to_string()),
            Some("s1".to_string()),
            Some("global".to_string()),
            None::<String>,
            Some("guid".to_string()),
            Some(1),
            Some("https://example.test/notestore".to_string()),
            None::<String>,
            None::<String>,
            None::<i32>,
        );
        let linked = linked_notebook_from_edam(linked);
        assert_eq!(linked.guid, "guid");
        assert_eq!(linked.share_name, "Shared");
        assert_eq!(linked.share_key, "global");
        assert_eq!(linked.note_store_url, "https://example.test/notestore");
    }
}
