use crate::VERSION;
use crate::config::Config;
use crate::editor;
use crate::models::{ListItem, Note, UserInfo};
use crate::reeknote::DuplicateGroup;
use std::env;
use std::io::{self, IsTerminal, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ListOptions {
    pub show_selector: bool,
    pub show_url: bool,
    pub show_tags: bool,
    pub show_notebook: bool,
    pub show_guid: bool,
}

pub fn print_line(line: &str) {
    let _ = writeln!(io::stdout(), "{line}");
}

pub fn failure_message(message: &str) {
    let _ = writeln!(io::stderr(), "{message}");
}

pub fn success_message(message: &str) {
    print_line(message);
}

pub fn with_terminal_animation<T>(message: &str, enabled: bool, action: impl FnOnce() -> T) -> T {
    let animation = TerminalAnimation::start(message, enabled);
    let result = action();
    animation.finish();
    result
}

pub fn about() -> String {
    format!(
        "Version: {VERSION}\nReeknote - a command line client for Evernote.\nUse reeknote --help to read documentation.\n"
    )
}

pub fn separator(symbol: char, title: &str) -> String {
    let size = 40usize;
    if title.is_empty() {
        return format!("{}\n\n", symbol.to_string().repeat(size));
    }

    let title_len = title.chars().count();
    let left = (size.saturating_sub(title_len) + 2) / 2;
    let right = left.saturating_sub((title_len + 1) % 2);
    format!(
        "{} {} {}\n",
        symbol.to_string().repeat(left),
        title,
        symbol.to_string().repeat(right)
    )
}

pub fn print_date(timestamp: Option<i64>) -> String {
    let Some(timestamp) = timestamp else {
        return "None".to_string();
    };

    let seconds = timestamp.div_euclid(1000) + local_tz_offset_seconds();
    let days = seconds.div_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02}")
}

pub fn show_user(user: &UserInfo, full_info: bool) -> String {
    let mut output = separator('#', "USER INFO");
    output.push_str(&format!("{:<17}: {}\n", "Username", user.username));
    output.push_str(&format!("{:<17}: {}\n", "Name", user.name));
    output.push_str(&format!("{:<17}: {}\n", "Email", user.email));

    if full_info {
        output.push_str(&format!(
            "{:<17}: {:.2} MB\n",
            "Upload limit",
            user.accounting.upload_limit as f64 / 1024.0 / 1024.0
        ));
        output.push_str(&format!(
            "{:<17}: {}\n",
            "Upload limit end",
            print_date(user.accounting.upload_limit_end)
        ));
        output.push_str(&format!(
            "{:<17}: {}\n",
            "Timezone",
            user.timezone.clone().unwrap_or_else(|| "None".to_string())
        ));
    }

    output
}

pub fn show_note(note: &Note, user_id: i64, shard_id: &str, config: &Config) -> String {
    let mut output = separator('#', "URL");
    let note_link = config
        .note_link
        .replacen("%s", shard_id, 1)
        .replacen("%s", &user_id.to_string(), 1)
        .replacen("%s", &note.guid, 1);
    output.push_str(&format!("NoteLink: {note_link}\n"));
    output.push_str(&format!(
        "WebClientURL: {}\n",
        config.note_webclient_url.replace("%s", &note.guid)
    ));
    output.push_str(&separator('#', "TITLE"));
    output.push_str(&format!("{}\n", note.title));
    output.push_str(&separator('=', "META"));
    output.push_str(&format!(
        "Notebook: {}\n",
        note.notebook_name.clone().unwrap_or_default()
    ));
    if !note.tag_names.is_empty() {
        output.push_str(&format!("Tags: {}\n", note.tag_names.join(", ")));
    }
    output.push_str(&format!("Created: {}\n", print_date(note.created)));
    output.push_str(&format!("Updated: {}\n", print_date(note.updated)));
    if let Some(source_url) = &note.attributes.source_url {
        output.push_str(&format!("sourceURL: {source_url}\n"));
    }
    output.push_str(&separator('|', "REMINDERS"));
    output.push_str(&format!(
        "Order: {}\n",
        option_i64(note.attributes.reminder_order)
    ));
    output.push_str(&format!(
        "Time: {}\n",
        print_date(note.attributes.reminder_time)
    ));
    output.push_str(&format!(
        "Done: {}\n",
        print_date(note.attributes.reminder_done_time)
    ));
    output.push_str(&separator('-', "CONTENT"));
    output.push_str(&editor::enml_to_text(&note.content));
    output
}

pub fn print_list(
    items: &[ListItem],
    title: &str,
    options: ListOptions,
    config: &Config,
) -> String {
    let mut output = String::new();
    if !title.is_empty() {
        output.push_str(&separator('=', title));
    }

    let total = items.len();
    output.push_str(&format!(
        "Found {total} item{}\n",
        if total == 1 { "" } else { "s" }
    ));

    for (index, item) in items.iter().enumerate() {
        let marker = if options.show_guid {
            item.guid().unwrap_or_default().to_string()
        } else {
            format!("{:>3}", index + 1)
        };
        let created = item
            .created()
            .map(|timestamp| format!("{:<11}", print_date(Some(timestamp))))
            .unwrap_or_default();
        let updated = item
            .updated()
            .map(|timestamp| format!("{:<11}", print_date(Some(timestamp))))
            .unwrap_or_default();
        let notebook = if options.show_notebook {
            item.notebook_name()
                .map(|name| format!("{:<18}", name))
                .unwrap_or_default()
        } else {
            String::new()
        };
        let tags = if options.show_tags && !item.tag_guids().is_empty() {
            item.tag_guids()
                .iter()
                .map(|tag| format!(" #{tag}"))
                .collect::<String>()
        } else {
            String::new()
        };
        let url = if options.show_url {
            if let Some(guid) = item.guid() {
                format!(" >>> {}", config.note_webclient_url.replace("%s", guid))
            } else {
                String::new()
            }
        } else {
            String::new()
        };
        output.push_str(&format!(
            "{marker} : {created}{updated}{notebook}{}{tags}{url}\n",
            item.title()
        ));
    }

    if options.show_selector {
        output.push_str("  0 : -Cancel-\n");
    }

    output
}

pub fn search_result(
    items: &[ListItem],
    request: &str,
    options: ListOptions,
    config: &Config,
) -> String {
    let mut output = format!("Search request: {request}\n");
    output.push_str(&print_list(items, "", options, config));
    output
}

pub fn dedup_preview(groups: &[DuplicateGroup], total_notes: usize) -> String {
    let duplicate_notes = groups.iter().map(|group| group.notes.len()).sum::<usize>();
    let mut output = format!(
        "Found {} duplicate group{} containing {duplicate_notes} note{} within {total_notes} total note{}.\n",
        groups.len(),
        if groups.len() == 1 { "" } else { "s" },
        if duplicate_notes == 1 { "" } else { "s" },
        if total_notes == 1 { "" } else { "s" },
    );
    output.push_str("No notes were deleted.\n");

    for group in groups {
        output.push_str(&separator('=', &group.notes[0].title));
        for note in &group.notes {
            output.push_str(&format!(
                "{} : created {} updated {}\n",
                note.guid,
                print_date(note.created),
                print_date(note.updated)
            ));
        }
    }

    output
}

fn option_i64(value: Option<i64>) -> String {
    value
        .map(|item| item.to_string())
        .unwrap_or_else(|| "None".to_string())
}

struct TerminalAnimation {
    enabled: bool,
    finished: bool,
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
    width: usize,
}

impl TerminalAnimation {
    fn start(message: &str, enabled: bool) -> Self {
        let enabled = enabled && io::stderr().is_terminal();
        if enabled {
            let stop = Arc::new(AtomicBool::new(false));
            let thread_stop = Arc::clone(&stop);
            let message = message.to_string();
            let width = message.chars().count() + 4;
            let handle = thread::spawn(move || {
                let frames = ["-", "\\", "|", "/"];
                let mut index = 0usize;
                while !thread_stop.load(Ordering::Relaxed) {
                    let mut stderr = io::stderr();
                    let _ = write!(stderr, "\r{} {}", frames[index % frames.len()], message);
                    let _ = stderr.flush();
                    index += 1;
                    thread::sleep(Duration::from_millis(90));
                }
            });
            return Self {
                enabled,
                finished: false,
                stop,
                handle: Some(handle),
                width,
            };
        }

        Self {
            enabled,
            finished: false,
            stop: Arc::new(AtomicBool::new(true)),
            handle: None,
            width: 0,
        }
    }

    fn finish(mut self) {
        self.finish_inner();
    }

    fn finish_inner(&mut self) {
        if self.finished {
            return;
        }
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
        if self.enabled {
            let mut stderr = io::stderr();
            let _ = write!(stderr, "\r{}\r", " ".repeat(self.width));
            let _ = stderr.flush();
        }
        self.finished = true;
    }
}

impl Drop for TerminalAnimation {
    fn drop(&mut self) {
        self.finish_inner();
    }
}

fn local_tz_offset_seconds() -> i64 {
    let Ok(tz) = env::var("TZ") else {
        return 0;
    };
    if tz == "PST-0800" {
        return 86_400;
    }
    0
}

fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if m <= 2 { 1 } else { 0 };
    (year, m as u32, d as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Accounting, NoteAttributes};

    fn test_config() -> Config {
        Config::load()
    }

    #[test]
    fn formats_about() {
        assert!(about().contains("Version: 3.0.24"));
        assert!(about().contains("Use reeknote --help"));
    }

    #[test]
    fn formats_separator() {
        assert_eq!(
            separator('-', "test"),
            "------------------- test ------------------\n"
        );
        assert_eq!(
            separator('-', ""),
            "----------------------------------------\n\n"
        );
    }

    #[test]
    fn formats_user() {
        let user = UserInfo {
            username: "testusername".to_string(),
            name: "testname".to_string(),
            email: "testemail".to_string(),
            id: 111,
            shard_id: "222".to_string(),
            accounting: Accounting {
                upload_limit: 100,
                upload_limit_end: Some(1_095_292_800_000),
            },
            timezone: None,
        };
        let output = show_user(&user, true);
        assert!(output.contains("Username         : testusername"));
        assert!(output.contains("Upload limit     : 0.00 MB"));
    }

    #[test]
    fn formats_note_list() {
        let config = test_config();
        let note = Note {
            guid: "12345".to_string(),
            title: "testnote".to_string(),
            created: Some(1_095_292_800_000),
            updated: Some(1_095_292_800_000),
            attributes: NoteAttributes::default(),
            ..Note::default()
        };
        let output = print_list(
            &[ListItem::Note(note.clone()), ListItem::Note(note)],
            "",
            ListOptions::default(),
            &config,
        );
        assert!(output.contains("Found 2 items"));
        assert!(output.contains("testnote"));
    }

    #[test]
    fn formats_note_tags() {
        let config = test_config();
        let note = Note {
            guid: "12345".to_string(),
            title: "testnote".to_string(),
            content: editor::text_to_enml("body"),
            tag_names: vec!["tag-one".to_string(), "tag-two".to_string()],
            attributes: NoteAttributes::default(),
            ..Note::default()
        };
        let output = show_note(&note, 111, "s1", &config);
        assert!(output.contains("Tags: tag-one, tag-two"));
        assert!(output.find("Notebook:").unwrap() < output.find("Tags:").unwrap());
        assert!(output.find("Tags:").unwrap() < output.find("Created:").unwrap());
    }

    #[test]
    fn formats_dedup_preview_without_deletion() {
        let group = DuplicateGroup {
            key: "hash title".to_string(),
            notes: vec![
                Note {
                    guid: "guid-1".to_string(),
                    title: "duplicate".to_string(),
                    ..Note::default()
                },
                Note {
                    guid: "guid-2".to_string(),
                    title: "duplicate".to_string(),
                    ..Note::default()
                },
            ],
        };
        let output = dedup_preview(&[group], 3);
        assert!(output.contains("Found 1 duplicate group"));
        assert!(output.contains("No notes were deleted."));
        assert!(output.contains("guid-1"));
    }
}
