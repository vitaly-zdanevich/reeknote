use crate::VERSION;
use crate::config::{self, Config};
use crate::editor::{self, TextFormat};
use crate::errors::{ReeknoteError, Result};
use crate::models::{LinkedNotebook, Note, Notebook, SearchResult, Tag, UserInfo};
use crate::storage::{Storage, StoredValue};
use crate::tools;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ParsedNoteInput {
    pub title: Option<String>,
    pub content: Option<String>,
    pub tags: Vec<String>,
    pub created: Option<i64>,
    pub notebook: Option<String>,
    pub resources: Vec<String>,
    pub reminder: Option<ReminderValue>,
    pub url: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReminderValue {
    Timestamp(i64),
    None,
    Done,
    Delete,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DuplicateGroup {
    pub key: String,
    pub notes: Vec<Note>,
}

pub trait EvernoteClient {
    fn get_user_info(&mut self) -> Result<UserInfo>;
    fn get_note(&mut self, guid: &str) -> Result<Note>;
    fn get_note_with_resources(&mut self, guid: &str) -> Result<Note>;
    fn get_note_content(&mut self, guid: &str) -> Result<String>;
    fn find_notes(&mut self, query: &str, count: usize, deleted_only: bool)
    -> Result<SearchResult>;
    fn create_note(&mut self, input: ParsedNoteInput) -> Result<Note>;
    fn update_note(&mut self, guid: &str, input: ParsedNoteInput) -> Result<()>;
    fn remove_note(&mut self, guid: &str) -> Result<()>;
    fn find_notebooks(&mut self) -> Result<Vec<Notebook>>;
    fn find_linked_notebooks(&mut self) -> Result<Vec<LinkedNotebook>>;
    fn create_notebook(&mut self, name: &str, stack: Option<&str>) -> Result<Notebook>;
    fn update_notebook(&mut self, guid: &str, name: &str) -> Result<()>;
    fn remove_notebook(&mut self, guid: &str) -> Result<()>;
    fn find_tags(&mut self) -> Result<Vec<Tag>>;
    fn create_tag(&mut self, name: &str) -> Result<Tag>;
    fn update_tag(&mut self, guid: &str, name: &str) -> Result<()>;
    fn remove_tag(&mut self, guid: &str) -> Result<()>;
}

#[derive(Clone, Debug, Default)]
pub struct UnsupportedEvernoteClient;

impl UnsupportedEvernoteClient {
    fn unsupported<T>(&self) -> Result<T> {
        Err(ReeknoteError::Unsupported(
            "Evernote Thrift transport is not implemented in this Rust build yet".to_string(),
        ))
    }
}

impl EvernoteClient for UnsupportedEvernoteClient {
    fn get_user_info(&mut self) -> Result<UserInfo> {
        self.unsupported()
    }

    fn get_note(&mut self, _guid: &str) -> Result<Note> {
        self.unsupported()
    }

    fn get_note_with_resources(&mut self, _guid: &str) -> Result<Note> {
        self.unsupported()
    }

    fn get_note_content(&mut self, _guid: &str) -> Result<String> {
        self.unsupported()
    }

    fn find_notes(
        &mut self,
        _query: &str,
        _count: usize,
        _deleted_only: bool,
    ) -> Result<SearchResult> {
        self.unsupported()
    }

    fn create_note(&mut self, _input: ParsedNoteInput) -> Result<Note> {
        self.unsupported()
    }

    fn update_note(&mut self, _guid: &str, _input: ParsedNoteInput) -> Result<()> {
        self.unsupported()
    }

    fn remove_note(&mut self, _guid: &str) -> Result<()> {
        self.unsupported()
    }

    fn find_notebooks(&mut self) -> Result<Vec<Notebook>> {
        self.unsupported()
    }

    fn find_linked_notebooks(&mut self) -> Result<Vec<LinkedNotebook>> {
        self.unsupported()
    }

    fn create_notebook(&mut self, _name: &str, _stack: Option<&str>) -> Result<Notebook> {
        self.unsupported()
    }

    fn update_notebook(&mut self, _guid: &str, _name: &str) -> Result<()> {
        self.unsupported()
    }

    fn remove_notebook(&mut self, _guid: &str) -> Result<()> {
        self.unsupported()
    }

    fn find_tags(&mut self) -> Result<Vec<Tag>> {
        self.unsupported()
    }

    fn create_tag(&mut self, _name: &str) -> Result<Tag> {
        self.unsupported()
    }

    fn update_tag(&mut self, _guid: &str, _name: &str) -> Result<()> {
        self.unsupported()
    }

    fn remove_tag(&mut self, _guid: &str) -> Result<()> {
        self.unsupported()
    }
}

pub fn get_editor(storage: Option<&Storage>) -> String {
    if let Some(editor) = storage
        .and_then(|storage| storage.get_userprop("editor"))
        .and_then(|value| value.as_string())
    {
        return editor.to_string();
    }
    if let Ok(editor) = std::env::var("editor") {
        if !editor.is_empty() {
            return editor;
        }
    }
    if let Ok(editor) = std::env::var("EDITOR") {
        if !editor.is_empty() {
            return editor;
        }
    }
    if cfg!(windows) {
        config::DEF_WIN_EDITOR.to_string()
    } else {
        config::DEF_UNIX_EDITOR.to_string()
    }
}

pub fn get_extras(storage: Option<&Storage>) -> Option<Vec<String>> {
    storage
        .and_then(|storage| storage.get_userprop("markdown2_extras"))
        .and_then(|value| value.as_string_list().map(|items| items.to_vec()))
}

pub fn get_note_ext(storage: &mut Storage) -> [String; 2] {
    if let Some(value) = storage
        .get_userprop("note_ext")
        .and_then(|value| value.as_string_list())
    {
        if value.len() == 2 {
            return [value[0].clone(), value[1].clone()];
        }
        let _ = storage.del_userprop("note_ext");
    }
    [
        config::DEFAULT_NOTE_EXT[0].to_string(),
        config::DEFAULT_NOTE_EXT[1].to_string(),
    ]
}

pub fn settings_output(storage: &mut Storage, config: &Config) -> String {
    let editor = get_editor(Some(storage));
    let extras = get_extras(Some(storage));
    let note_ext = get_note_ext(storage);
    let mut lines = vec![
        "Geeknote".to_string(),
        "*".repeat(30),
        format!("Version: {VERSION}"),
        format!("App dir: {}", config.app_dir.display()),
        format!("Error log: {}", config.error_log.display()),
        format!("Editor: {editor}"),
        format!("Markdown2 Extras: {:?}", extras),
        format!("Note extension: {:?}", note_ext),
    ];

    if let Some(user) = storage.get_user_info() {
        lines.push("*".repeat(30));
        lines.push(format!("Username: {}", user.username));
        lines.push(format!("Id: {}", user.id));
        lines.push(format!("Email: {}", user.email));
    }
    lines.join("\n")
}

pub struct NotesService;

impl NotesService {
    #[allow(clippy::too_many_arguments)]
    pub fn parse_input(
        title: Option<String>,
        content: Option<String>,
        tags: Vec<String>,
        created: Option<String>,
        notebook: Option<String>,
        resources: Vec<String>,
        note: Option<&Note>,
        reminder: Option<String>,
        url: Option<String>,
        rawmd: bool,
    ) -> Result<ParsedNoteInput> {
        let mut result = ParsedNoteInput {
            title: title.map(|value| tools::strip_string(&value)),
            content,
            tags: tools::strip_vec(&tags),
            created: None,
            notebook: notebook.map(|value| tools::strip_string(&value)),
            resources: tools::strip_vec(&resources),
            reminder: None,
            url,
        };

        if result.title.is_none() {
            if let Some(note) = note {
                result.title = Some(note.title.clone());
            }
        }

        if result.content.is_none()
            && note.is_some()
            && tags.is_empty()
            && created.is_none()
            && result.notebook.is_none()
            && resources.is_empty()
            && reminder.is_none()
            && result.url.is_none()
        {
            result.content = Some(config::EDITOR_OPEN.to_string());
        }

        if let Some(content) = result.content.clone() {
            if content != config::EDITOR_OPEN {
                let loaded = if Path::new(&content).is_file() {
                    fs::read_to_string(&content)?
                } else {
                    content
                };
                result.content = Some(editor::text_to_enml_with_options(
                    &loaded,
                    TextFormat::Markdown,
                    rawmd,
                ));
            }
        }

        if let Some(created) = created {
            result.created = Some(get_time_from_date(&created)?);
        }

        if let Some(reminder) = reminder {
            result.reminder = Some(parse_reminder(&reminder)?);
        }

        if result.url.is_none() {
            if let Some(note) = note {
                result.url = note.attributes.source_url.clone();
            }
        }

        Ok(result)
    }

    pub fn create_search_request(
        search: Option<&str>,
        tags: &[String],
        notebook: Option<&str>,
        date: Option<&str>,
        exact_entry: bool,
        content_search: bool,
        ignore_completed: bool,
        reminders_only: bool,
    ) -> Result<String> {
        let mut request = String::new();

        if let Some(notebook) = notebook {
            request.push_str(&format_expression("notebook", notebook));
        }

        for tag in tags {
            request.push_str(&format_expression("tag", tag));
        }

        if let Some(date) = date {
            let parts = date
                .split(config::DEF_DATE_RANGE_DELIMITER)
                .map(tools::strip_string)
                .collect::<Vec<_>>();
            if parts.is_empty() || parts.len() > 2 {
                return Err(ReeknoteError::InvalidInput(format!(
                    "incorrect date format ({date}) in --date attribute"
                )));
            }
            request.push_str(&format!(
                "created:{} ",
                format_evernote_time(get_time_from_date(&parts[0])?)
            ));
            if parts.len() == 2 {
                request.push_str(&format!(
                    "-created:{} ",
                    format_evernote_time(get_time_from_date(&parts[1])? + 86_400_000)
                ));
            }
        }

        if let Some(search) = search {
            let mut search = tools::strip_string(search);
            if exact_entry {
                search = format!("\"{search}\"");
            }
            if content_search {
                request.push_str(&search);
            } else {
                request.push_str(&format!("intitle:{search}"));
            }
        }

        if reminders_only {
            request.push_str(" reminderOrder:* ");
        }
        if ignore_completed {
            request.push_str(" -reminderDoneTime:* ");
        }

        Ok(request)
    }
}

fn parse_reminder(reminder: &str) -> Result<ReminderValue> {
    if let Some(offset) = config::reminder_shortcut(reminder) {
        return Ok(ReminderValue::Timestamp(now_millis() + offset));
    }
    match reminder {
        config::REMINDER_NONE => Ok(ReminderValue::None),
        config::REMINDER_DONE => Ok(ReminderValue::Done),
        config::REMINDER_DELETE => Ok(ReminderValue::Delete),
        _ => Ok(ReminderValue::Timestamp(get_time_from_date(reminder)?)),
    }
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or_default()
}

fn format_expression(label: &str, value: &str) -> String {
    let mut expression = String::new();
    let mut value = value.to_string();
    if let Some(stripped) = value.strip_prefix('-') {
        expression.push('-');
        value = stripped.to_string();
    }
    value = tools::strip_string(&value);
    if value.contains(' ') {
        value = format!("\"{value}\"");
    }
    expression.push_str(&format!("{label}:{value} "));
    expression
}

pub fn get_time_from_date(date: &str) -> Result<i64> {
    if let Some((date_part, time_part)) = date.split_once(' ') {
        let (year, month, day) = parse_date(date_part)?;
        let (hour, minute) = parse_time(time_part)?;
        return Ok(
            (days_from_civil(year, month, day) * 86_400 + hour * 3_600 + minute * 60 + 1) * 1000,
        );
    }

    let (year, month, day) = parse_date(date)?;
    Ok((days_from_civil(year, month, day) * 86_400 + 1) * 1000)
}

fn parse_date(date: &str) -> Result<(i64, i64, i64)> {
    let parts = date.split('-').collect::<Vec<_>>();
    if parts.len() != 3 {
        return Err(ReeknoteError::InvalidInput(format!(
            "incorrect date format: {date}"
        )));
    }
    let year = parts[0]
        .parse::<i64>()
        .map_err(|_| ReeknoteError::InvalidInput(format!("incorrect date format: {date}")))?;
    let month = parts[1]
        .parse::<i64>()
        .map_err(|_| ReeknoteError::InvalidInput(format!("incorrect date format: {date}")))?;
    let day = parts[2]
        .parse::<i64>()
        .map_err(|_| ReeknoteError::InvalidInput(format!("incorrect date format: {date}")))?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return Err(ReeknoteError::InvalidInput(format!(
            "incorrect date format: {date}"
        )));
    }
    Ok((year, month, day))
}

fn parse_time(time: &str) -> Result<(i64, i64)> {
    let parts = time.split(':').collect::<Vec<_>>();
    if parts.len() != 2 {
        return Err(ReeknoteError::InvalidInput(format!(
            "incorrect time format: {time}"
        )));
    }
    let hour = parts[0]
        .parse::<i64>()
        .map_err(|_| ReeknoteError::InvalidInput(format!("incorrect time format: {time}")))?;
    let minute = parts[1]
        .parse::<i64>()
        .map_err(|_| ReeknoteError::InvalidInput(format!("incorrect time format: {time}")))?;
    if !(0..=23).contains(&hour) || !(0..=59).contains(&minute) {
        return Err(ReeknoteError::InvalidInput(format!(
            "incorrect time format: {time}"
        )));
    }
    Ok((hour, minute))
}

fn format_evernote_time(timestamp_ms: i64) -> String {
    let seconds = timestamp_ms.div_euclid(1000);
    let days = seconds.div_euclid(86_400);
    let seconds_of_day = seconds.rem_euclid(86_400);
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}{month:02}{day:02}T{hour:02}{minute:02}00Z")
}

fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = year - i64::from(month <= 2);
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let yoe = year - era * 400;
    let month = month + if month > 2 { -3 } else { 9 };
    let doy = (153 * month + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    let y = y + if m <= 2 { 1 } else { 0 };
    (y, m, d)
}

pub fn set_editor(storage: &mut Storage, editor: &str) -> Result<()> {
    storage.set_userprop("editor", StoredValue::String(editor.to_string()))
}

pub fn set_extras(storage: &mut Storage, extras: &str) -> Result<()> {
    let extras = extras
        .split(',')
        .map(tools::strip_string)
        .collect::<Vec<_>>();
    storage.set_userprop("markdown2_extras", StoredValue::StringList(extras))
}

pub fn set_note_ext(storage: &mut Storage, note_ext: &str) -> Result<()> {
    let values = note_ext
        .split(',')
        .map(tools::strip_string)
        .collect::<Vec<_>>();
    if values.len() == 2 {
        storage.set_userprop("note_ext", StoredValue::StringList(values))
    } else {
        Err(ReeknoteError::InvalidInput(
            "note extension format is '.markdown_extension, .raw_extension'".to_string(),
        ))
    }
}

pub fn duplicate_metadata_groups(notes: &[Note]) -> Vec<Vec<Note>> {
    let mut groups: BTreeMap<String, Vec<Note>> = BTreeMap::new();
    for note in notes {
        groups
            .entry(duplicate_metadata_key(note))
            .or_default()
            .push(note.clone());
    }
    groups
        .into_values()
        .filter(|notes| notes.len() > 1)
        .collect()
}

pub fn duplicate_content_groups(notes: Vec<Note>) -> Vec<DuplicateGroup> {
    let mut groups: BTreeMap<String, Vec<Note>> = BTreeMap::new();
    for note in notes {
        groups
            .entry(duplicate_content_key(&note))
            .or_default()
            .push(note);
    }
    groups
        .into_iter()
        .filter(|(_, notes)| notes.len() > 1)
        .map(|(key, notes)| DuplicateGroup { key, notes })
        .collect()
}

fn duplicate_metadata_key(note: &Note) -> String {
    format!(
        "{} ({:?}) with {:?} ({:?})",
        note.title, note.content_length, note.largest_resource_mime, note.largest_resource_size
    )
}

fn duplicate_content_key(note: &Note) -> String {
    format!("{:x} {}", md5::compute(note.content.as_bytes()), note.title)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_search_request() {
        let tags = vec!["tag1".to_string(), "tag2".to_string()];
        let request = NotesService::create_search_request(
            Some("test text"),
            &tags,
            Some("notebook1"),
            Some("1999-12-31/2000-12-31"),
            false,
            false,
            false,
            false,
        )
        .unwrap();
        assert_eq!(
            request,
            "notebook:notebook1 tag:tag1 tag:tag2 created:19991231T000000Z -created:20010101T000000Z intitle:test text"
        );
    }

    #[test]
    fn parses_note_input() {
        let input = NotesService::parse_input(
            Some("title".to_string()),
            Some("test body".to_string()),
            vec!["tag1".to_string()],
            None,
            None,
            vec!["res 1".to_string()],
            None,
            None,
            None,
            false,
        )
        .unwrap();
        assert_eq!(input.title.as_deref(), Some("title"));
        assert_eq!(
            input.content.as_deref(),
            Some(editor::text_to_enml("test body").as_str())
        );
        assert_eq!(input.tags, vec!["tag1"]);
        assert_eq!(input.resources, vec!["res 1"]);
    }

    #[test]
    fn handles_settings_helpers() {
        let mut storage = Storage::memory();
        assert!(!get_editor(Some(&storage)).is_empty());
        set_editor(&mut storage, "vim").unwrap();
        assert_eq!(get_editor(Some(&storage)), "vim");
        set_note_ext(&mut storage, ".md,.enml").unwrap();
        assert_eq!(
            get_note_ext(&mut storage),
            [".md".to_string(), ".enml".to_string()]
        );
    }

    #[test]
    fn finds_duplicate_content_groups_from_metadata_candidates() {
        let notes = vec![
            Note {
                guid: "1".to_string(),
                title: "same".to_string(),
                content: "body".to_string(),
                content_length: Some(4),
                ..Note::default()
            },
            Note {
                guid: "2".to_string(),
                title: "same".to_string(),
                content: "body".to_string(),
                content_length: Some(4),
                ..Note::default()
            },
            Note {
                guid: "3".to_string(),
                title: "same".to_string(),
                content: "other".to_string(),
                content_length: Some(5),
                ..Note::default()
            },
        ];
        let candidates = duplicate_metadata_groups(&notes);
        assert_eq!(candidates.len(), 1);
        let groups = duplicate_content_groups(candidates.into_iter().flatten().collect());
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].notes.len(), 2);
    }
}
