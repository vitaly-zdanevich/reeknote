use std::env;
use std::fs;
use std::path::PathBuf;

pub const DEF_UNIX_EDITOR: &str = "nano";
pub const DEF_WIN_EDITOR: &str = "notepad.exe";
pub const EDITOR_OPEN: &str = "WRITE";

pub const REMINDER_NONE: &str = "NONE";
pub const REMINDER_DONE: &str = "DONE";
pub const REMINDER_DELETE: &str = "DELETE";

pub const DEF_DATE_FORMAT: &str = "%Y-%m-%d";
pub const DEF_DATE_AND_TIME_FORMAT: &str = "%Y-%m-%d %H:%M";
pub const DEF_DATE_RANGE_DELIMITER: char = '/';

pub const DEFAULT_NOTE_EXT: [&str; 2] = [".markdown", ".org"];
pub const MARKDOWN_EXTENSIONS: [&str; 2] = [".md", ".markdown"];
pub const HTML_EXTENSIONS: [&str; 2] = [".html", ".org"];

pub const CONSUMER_KEY: &str = "skaizer-5314";
pub const CONSUMER_SECRET: &str = "6f4f9183b3120801";
pub const NOTE_SORT_ORDER: &str = "UPDATED";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    pub app_dir: PathBuf,
    pub error_log: PathBuf,
    pub user_base_url: String,
    pub user_store_uri: String,
    pub note_webclient_url: String,
    pub note_link: String,
}

impl Config {
    pub fn load() -> Self {
        let home = env::var_os("HOME")
            .or_else(|| env::var_os("USERPROFILE"))
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        let app_dir = env::var_os("REEKNOTE_APP_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".reeknote"));

        if app_dir.exists() {
            // Keep going if this is a file: later storage operations will report
            // the concrete error from the filesystem.
        } else {
            let _ = fs::create_dir_all(&app_dir);
        }

        let user_base_url = if env::var("REEKNOTE_BASE").ok().as_deref() == Some("yinxiang")
            || app_dir.join("isyinxiang").is_file()
        {
            "app.yinxiang.com".to_string()
        } else {
            "www.evernote.com".to_string()
        };

        let user_store_uri = format!("https://{user_base_url}/edam/user");
        let note_webclient_url = format!("https://{user_base_url}/Home.action?#n=%s");
        let note_link = format!("https://{user_base_url}/shard/%s/nl/%s/%s");
        let error_log = app_dir.join("error.log");

        Self {
            app_dir,
            error_log,
            user_base_url,
            user_store_uri,
            note_webclient_url,
            note_link,
        }
    }
}

pub fn reminder_shortcut(value: &str) -> Option<i64> {
    match value {
        "TOMORROW" => Some(86_400_000),
        "WEEK" => Some(604_800_000),
        _ => None,
    }
}
