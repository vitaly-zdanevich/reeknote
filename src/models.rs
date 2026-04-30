#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Accounting {
    pub upload_limit: u64,
    pub upload_limit_end: Option<i64>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UserInfo {
    pub username: String,
    pub name: String,
    pub email: String,
    pub id: i64,
    pub shard_id: String,
    pub accounting: Accounting,
    pub timezone: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NoteAttributes {
    pub source_url: Option<String>,
    pub reminder_order: Option<i64>,
    pub reminder_time: Option<i64>,
    pub reminder_done_time: Option<i64>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Note {
    pub guid: String,
    pub title: String,
    pub content: String,
    pub created: Option<i64>,
    pub updated: Option<i64>,
    pub notebook_guid: Option<String>,
    pub notebook_name: Option<String>,
    pub tag_guids: Vec<String>,
    pub tag_names: Vec<String>,
    pub attributes: NoteAttributes,
    pub resources: Vec<Resource>,
    pub content_length: Option<usize>,
    pub largest_resource_mime: Option<String>,
    pub largest_resource_size: Option<usize>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Notebook {
    pub guid: String,
    pub name: String,
    pub stack: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LinkedNotebook {
    pub guid: String,
    pub share_name: String,
    pub share_key: String,
    pub shard_id: String,
    pub note_store_url: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tag {
    pub guid: String,
    pub name: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SearchResult {
    pub total_notes: usize,
    pub notes: Vec<Note>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ResourceData {
    pub body_hash: String,
    pub body: Vec<u8>,
    pub size: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Resource {
    pub mime: Option<String>,
    pub filename: String,
    pub data: ResourceData,
}

#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ListItem {
    Note(Note),
    Notebook(Notebook),
    LinkedNotebook(LinkedNotebook),
    Tag(Tag),
}

impl ListItem {
    pub fn guid(&self) -> Option<&str> {
        match self {
            Self::Note(item) => Some(&item.guid),
            Self::Notebook(item) => Some(&item.guid),
            Self::LinkedNotebook(item) => Some(&item.guid),
            Self::Tag(item) => Some(&item.guid),
        }
    }

    pub fn created(&self) -> Option<i64> {
        match self {
            Self::Note(item) => item.created,
            _ => None,
        }
    }

    pub fn updated(&self) -> Option<i64> {
        match self {
            Self::Note(item) => item.updated,
            _ => None,
        }
    }

    pub fn notebook_name(&self) -> Option<&str> {
        match self {
            Self::Note(item) => item.notebook_name.as_deref(),
            _ => None,
        }
    }

    pub fn title(&self) -> &str {
        match self {
            Self::Note(item) => &item.title,
            Self::Notebook(item) => &item.name,
            Self::LinkedNotebook(item) => &item.share_name,
            Self::Tag(item) => &item.name,
        }
    }

    pub fn tag_guids(&self) -> &[String] {
        match self {
            Self::Note(item) => &item.tag_guids,
            _ => &[],
        }
    }
}
