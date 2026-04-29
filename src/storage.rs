use crate::errors::{ReeknoteError, Result};
use crate::models::{Note, SearchResult, UserInfo};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StoredValue {
    String(String),
    StringList(Vec<String>),
    UserInfo(UserInfo),
}

impl StoredValue {
    pub fn as_string(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_string_list(&self) -> Option<&[String]> {
        match self {
            Self::StringList(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_user_info(&self) -> Option<&UserInfo> {
        match self {
            Self::UserInfo(value) => Some(value),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct StorageData {
    user_props: BTreeMap<String, StoredValue>,
    settings: BTreeMap<String, String>,
    tags: BTreeMap<String, String>,
    notebooks: BTreeMap<String, String>,
    notes: BTreeMap<String, Note>,
    search: Option<SearchResult>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Storage {
    path: Option<PathBuf>,
    data: StorageData,
}

impl Storage {
    pub fn memory() -> Self {
        Self {
            path: None,
            data: StorageData::default(),
        }
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let data = if path.is_file() {
            parse_storage(&fs::read_to_string(&path)?)?
        } else {
            StorageData::default()
        };
        Ok(Self {
            path: Some(path),
            data,
        })
    }

    pub fn create_user(&mut self, oauth_token: impl Into<String>, info: UserInfo) -> Result<()> {
        let oauth_token = oauth_token.into();
        if oauth_token.is_empty() {
            return Err(ReeknoteError::InvalidInput("empty OAuth token".to_string()));
        }
        if info.email.is_empty() && info.username.is_empty() {
            return Err(ReeknoteError::InvalidInput("empty user info".to_string()));
        }

        self.data.user_props.clear();
        self.data
            .user_props
            .insert("oAuthToken".to_string(), StoredValue::String(oauth_token));
        self.data
            .user_props
            .insert("info".to_string(), StoredValue::UserInfo(info));
        self.persist()
    }

    pub fn remove_user(&mut self) -> Result<()> {
        self.data.user_props.clear();
        self.persist()
    }

    pub fn get_user_token(&self) -> Option<String> {
        self.get_userprop("oAuthToken")
            .and_then(|value| value.as_string().map(ToOwned::to_owned))
    }

    pub fn get_user_info(&self) -> Option<UserInfo> {
        self.get_userprop("info")
            .and_then(|value| value.as_user_info().cloned())
    }

    pub fn get_userprops(&self) -> Vec<BTreeMap<String, StoredValue>> {
        self.data
            .user_props
            .iter()
            .map(|(key, value)| BTreeMap::from([(key.clone(), value.clone())]))
            .collect()
    }

    pub fn get_userprop(&self, key: &str) -> Option<&StoredValue> {
        self.data.user_props.get(key)
    }

    pub fn set_userprop(&mut self, key: impl Into<String>, value: StoredValue) -> Result<()> {
        self.data.user_props.insert(key.into(), value);
        self.persist()
    }

    pub fn del_userprop(&mut self, key: &str) -> Result<bool> {
        let removed = self.data.user_props.remove(key).is_some();
        self.persist()?;
        Ok(removed)
    }

    pub fn set_settings(&mut self, settings: BTreeMap<String, String>) -> Result<()> {
        for (key, value) in settings {
            if key.is_empty() || value.is_empty() {
                return Err(ReeknoteError::InvalidInput(
                    "wrong setting item".to_string(),
                ));
            }
            self.data.settings.insert(key, value);
        }
        self.persist()
    }

    pub fn get_settings(&self) -> BTreeMap<String, String> {
        self.data.settings.clone()
    }

    pub fn set_setting(&mut self, key: impl Into<String>, value: impl Into<String>) -> Result<()> {
        self.data.settings.insert(key.into(), value.into());
        self.persist()
    }

    pub fn get_setting(&self, key: &str) -> Option<String> {
        self.data.settings.get(key).cloned()
    }

    pub fn set_tags(&mut self, tags: BTreeMap<String, String>) -> Result<()> {
        if tags.values().any(|value| value.is_empty()) {
            return Err(ReeknoteError::InvalidInput("wrong tag item".to_string()));
        }
        self.data.tags = tags;
        self.persist()
    }

    pub fn get_tags(&self) -> BTreeMap<String, String> {
        self.data.tags.clone()
    }

    pub fn set_notebooks(&mut self, notebooks: BTreeMap<String, String>) -> Result<()> {
        if notebooks.values().any(|value| value.is_empty()) {
            return Err(ReeknoteError::InvalidInput(
                "wrong notebook item".to_string(),
            ));
        }
        self.data.notebooks = notebooks;
        self.persist()
    }

    pub fn get_notebooks(&self) -> BTreeMap<String, String> {
        self.data.notebooks.clone()
    }

    pub fn set_note(&mut self, note: Note) -> Result<()> {
        self.data.notes.insert(note.guid.clone(), note);
        self.persist()
    }

    pub fn get_note(&self, guid: &str) -> Option<Note> {
        self.data.notes.get(guid).cloned()
    }

    pub fn set_search(&mut self, search: SearchResult) -> Result<()> {
        self.data.search = Some(search);
        self.persist()
    }

    pub fn get_search(&self) -> Option<SearchResult> {
        self.data.search.clone()
    }

    fn persist(&self) -> Result<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, serialize_storage(&self.data))?;
        Ok(())
    }
}

fn serialize_storage(data: &StorageData) -> String {
    let mut lines = Vec::new();
    for (key, value) in &data.user_props {
        match value {
            StoredValue::String(value) => {
                lines.push(format!("user.string\t{}\t{}", enc(key), enc(value)))
            }
            StoredValue::StringList(values) => {
                let encoded = values
                    .iter()
                    .map(|item| enc(item))
                    .collect::<Vec<_>>()
                    .join("\t");
                lines.push(format!("user.list\t{}\t{}", enc(key), encoded));
            }
            StoredValue::UserInfo(info) => lines.push(format!(
                "user.info\t{}\t{}\t{}\t{}\t{}\t{}",
                enc(key),
                enc(&info.username),
                enc(&info.name),
                enc(&info.email),
                info.id,
                enc(&info.shard_id)
            )),
        }
    }
    for (key, value) in &data.settings {
        lines.push(format!("setting\t{}\t{}", enc(key), enc(value)));
    }
    for (key, value) in &data.tags {
        lines.push(format!("tag\t{}\t{}", enc(key), enc(value)));
    }
    for (key, value) in &data.notebooks {
        lines.push(format!("notebook\t{}\t{}", enc(key), enc(value)));
    }
    for (guid, note) in &data.notes {
        lines.push(format!(
            "note\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            enc(guid),
            enc(&note.title),
            note.created
                .map(|value| value.to_string())
                .unwrap_or_default(),
            note.updated
                .map(|value| value.to_string())
                .unwrap_or_default(),
            enc(note.notebook_guid.as_deref().unwrap_or_default()),
            enc(note.notebook_name.as_deref().unwrap_or_default()),
            note.content_length
                .map(|value| value.to_string())
                .unwrap_or_default(),
            enc(note.largest_resource_mime.as_deref().unwrap_or_default()),
            note.largest_resource_size
                .map(|value| value.to_string())
                .unwrap_or_default(),
            note.tag_guids
                .iter()
                .map(|tag| enc(tag))
                .collect::<Vec<_>>()
                .join(",")
        ));
    }
    if let Some(search) = &data.search {
        lines.push(format!(
            "search\t{}",
            search
                .notes
                .iter()
                .map(|note| enc(&note.guid))
                .collect::<Vec<_>>()
                .join("\t")
        ));
    }
    lines.push(String::new());
    lines.join("\n")
}

fn parse_storage(content: &str) -> Result<StorageData> {
    let mut data = StorageData::default();
    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split('\t').collect();
        match parts.as_slice() {
            ["user.string", key, value] => {
                data.user_props
                    .insert(dec(key)?, StoredValue::String(dec(value)?));
            }
            ["user.list", key, values @ ..] => {
                let values = values
                    .iter()
                    .map(|item| dec(item))
                    .collect::<Result<Vec<_>>>()?;
                data.user_props
                    .insert(dec(key)?, StoredValue::StringList(values));
            }
            ["user.info", key, username, name, email, id, shard_id] => {
                data.user_props.insert(
                    dec(key)?,
                    StoredValue::UserInfo(UserInfo {
                        username: dec(username)?,
                        name: dec(name)?,
                        email: dec(email)?,
                        id: id.parse().unwrap_or_default(),
                        shard_id: dec(shard_id)?,
                        ..UserInfo::default()
                    }),
                );
            }
            ["setting", key, value] => {
                data.settings.insert(dec(key)?, dec(value)?);
            }
            ["tag", key, value] => {
                data.tags.insert(dec(key)?, dec(value)?);
            }
            ["notebook", key, value] => {
                data.notebooks.insert(dec(key)?, dec(value)?);
            }
            [
                "note",
                guid,
                title,
                created,
                updated,
                notebook_guid,
                notebook_name,
                content_length,
                largest_resource_mime,
                largest_resource_size,
                tag_guids,
            ] => {
                let guid = dec(guid)?;
                let tag_guids = if tag_guids.is_empty() {
                    Vec::new()
                } else {
                    tag_guids.split(',').map(dec).collect::<Result<Vec<_>>>()?
                };
                data.notes.insert(
                    guid.clone(),
                    Note {
                        guid,
                        title: dec(title)?,
                        created: parse_optional_i64(created),
                        updated: parse_optional_i64(updated),
                        notebook_guid: parse_optional_string(notebook_guid)?,
                        notebook_name: parse_optional_string(notebook_name)?,
                        content_length: parse_optional_usize(content_length),
                        largest_resource_mime: parse_optional_string(largest_resource_mime)?,
                        largest_resource_size: parse_optional_usize(largest_resource_size),
                        tag_guids,
                        ..Note::default()
                    },
                );
            }
            ["search", guids @ ..] => {
                let notes = guids
                    .iter()
                    .filter_map(|guid| dec(guid).ok())
                    .filter_map(|guid| data.notes.get(&guid).cloned())
                    .collect::<Vec<_>>();
                data.search = Some(SearchResult {
                    total_notes: notes.len(),
                    notes,
                });
            }
            _ => {
                return Err(ReeknoteError::Parse(format!(
                    "invalid storage line: {line}"
                )));
            }
        }
    }
    Ok(data)
}

fn parse_optional_string(value: &str) -> Result<Option<String>> {
    let value = dec(value)?;
    if value.is_empty() {
        Ok(None)
    } else {
        Ok(Some(value))
    }
}

fn parse_optional_i64(value: &str) -> Option<i64> {
    if value.is_empty() {
        None
    } else {
        value.parse().ok()
    }
}

fn parse_optional_usize(value: &str) -> Option<usize> {
    if value.is_empty() {
        None
    } else {
        value.parse().ok()
    }
}

fn enc(value: &str) -> String {
    let mut output = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.') {
            output.push(byte as char);
        } else {
            output.push_str(&format!("%{byte:02X}"));
        }
    }
    output
}

fn dec(value: &str) -> Result<String> {
    let mut bytes = Vec::new();
    let mut chars = value.as_bytes().iter().copied().peekable();
    while let Some(byte) = chars.next() {
        if byte == b'%' {
            let hi = chars
                .next()
                .ok_or_else(|| ReeknoteError::Parse("bad percent escape".to_string()))?;
            let lo = chars
                .next()
                .ok_or_else(|| ReeknoteError::Parse("bad percent escape".to_string()))?;
            let hex = [hi, lo];
            let hex = std::str::from_utf8(&hex)
                .map_err(|_| ReeknoteError::Parse("bad percent escape".to_string()))?;
            let decoded = u8::from_str_radix(hex, 16)
                .map_err(|_| ReeknoteError::Parse("bad percent escape".to_string()))?;
            bytes.push(decoded);
        } else {
            bytes.push(byte);
        }
    }
    String::from_utf8(bytes)
        .map_err(|_| ReeknoteError::Parse("invalid UTF-8 in storage".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user() -> UserInfo {
        UserInfo {
            username: "tester".to_string(),
            email: "test@mail.com".to_string(),
            ..UserInfo::default()
        }
    }

    #[test]
    fn stores_user() {
        let mut storage = Storage::memory();
        storage.create_user("token", user()).unwrap();
        assert_eq!(storage.get_user_token(), Some("token".to_string()));
        assert_eq!(storage.get_user_info().unwrap().email, "test@mail.com");
    }

    #[test]
    fn rejects_empty_user_values() {
        let mut storage = Storage::memory();
        assert!(storage.create_user("", user()).is_err());
        assert!(storage.create_user("token", UserInfo::default()).is_err());
    }

    #[test]
    fn stores_settings_tags_notebooks_and_search() {
        let mut storage = Storage::memory();
        storage.set_setting("editor", "vim").unwrap();
        assert_eq!(storage.get_setting("editor"), Some("vim".to_string()));

        storage
            .set_tags(BTreeMap::from([("guid".to_string(), "tag".to_string())]))
            .unwrap();
        assert_eq!(storage.get_tags()["guid"], "tag");

        storage
            .set_notebooks(BTreeMap::from([(
                "guid".to_string(),
                "notebook".to_string(),
            )]))
            .unwrap();
        assert_eq!(storage.get_notebooks()["guid"], "notebook");

        storage
            .set_search(SearchResult {
                total_notes: 0,
                notes: Vec::new(),
            })
            .unwrap();
        assert_eq!(storage.get_search().unwrap().total_notes, 0);
    }

    #[test]
    fn persists_simple_values() {
        let path = std::env::temp_dir().join(format!("reeknote-storage-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let mut storage = Storage::open(&path).unwrap();
        storage.create_user("token", user()).unwrap();
        storage.set_setting("editor", "nano").unwrap();

        let loaded = Storage::open(&path).unwrap();
        assert_eq!(loaded.get_user_token(), Some("token".to_string()));
        assert_eq!(loaded.get_setting("editor"), Some("nano".to_string()));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn persists_search_notes() {
        let path =
            std::env::temp_dir().join(format!("reeknote-storage-search-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let mut storage = Storage::open(&path).unwrap();
        let note = Note {
            guid: "guid".to_string(),
            title: "title".to_string(),
            created: Some(1),
            tag_guids: vec!["tag".to_string()],
            ..Note::default()
        };
        storage.set_note(note.clone()).unwrap();
        storage
            .set_search(SearchResult {
                total_notes: 1,
                notes: vec![note],
            })
            .unwrap();

        let loaded = Storage::open(&path).unwrap();
        let search = loaded.get_search().unwrap();
        assert_eq!(search.notes[0].guid, "guid");
        assert_eq!(search.notes[0].tag_guids, vec!["tag"]);
        let _ = std::fs::remove_file(path);
    }
}
