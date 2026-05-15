use crate::editor::{self, ImageOptions, TextFormat};
use crate::errors::Result;
use crate::models::{Note, Resource};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub fn remove_control_characters(value: &str) -> String {
    value
        .chars()
        .filter(|character| {
            let code = *character as u32;
            !((0x00..=0x08).contains(&code)
                || (0x0e..=0x1f).contains(&code)
                || (0x7f..=0x9f).contains(&code))
        })
        .collect()
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum SyncFormat {
    #[default]
    Plain,
    Markdown,
    Html,
}

impl SyncFormat {
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Plain => ".txt",
            Self::Markdown => ".md",
            Self::Html => ".html",
        }
    }

    pub fn text_format(&self) -> TextFormat {
        match self {
            Self::Plain => TextFormat::Plain,
            Self::Markdown => TextFormat::Markdown,
            Self::Html => TextFormat::Html,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyncFile {
    pub path: PathBuf,
    pub name: String,
    pub mtime_ms: i64,
}

pub fn parse_meta(content: &str) -> BTreeMap<String, String> {
    let mut result = BTreeMap::new();
    if let Some(rest) = content.strip_prefix("---")
        && let Some(end) = rest.find("---")
    {
        let meta = &rest[..end];
        for line in meta.lines() {
            if let Some((key, value)) = line.split_once(':') {
                result.insert(key.trim().to_string(), value.trim().to_string());
            }
        }
        result.insert("content".to_string(), rest[end + 3..].to_string());
        return result;
    }
    result.insert("content".to_string(), content.to_string());
    result
}

pub fn files_matching(path: &Path, mask: &str) -> Result<Vec<SyncFile>> {
    let mut files = Vec::new();
    let extension_filter = mask
        .strip_prefix("*.")
        .map(|extension| format!(".{extension}"));

    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            if let Some(extension_filter) = &extension_filter
                && path
                    .extension()
                    .and_then(|extension| extension.to_str())
                    .map(|extension| format!(".{extension}"))
                    != Some(extension_filter.clone())
            {
                continue;
            }
            let metadata = entry.metadata()?;
            let mtime_ms = metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|duration| duration.as_millis() as i64)
                .unwrap_or_default();
            let name = path
                .file_stem()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
                .to_string();
            files.push(SyncFile {
                path,
                name,
                mtime_ms,
            });
        }
    }

    Ok(files)
}

pub fn create_file_from_note(
    note: &Note,
    path: &Path,
    format: SyncFormat,
    image_options: &ImageOptions,
) -> Result<PathBuf> {
    fs::create_dir_all(path)?;
    let escaped_title = escape_path_component(&note.title);
    let mut image_options = image_options.clone();
    if image_options.save_images {
        image_options.base_filename = Some(if image_options.images_in_subdir {
            format!("{escaped_title}_images/{escaped_title}")
        } else {
            escaped_title.clone()
        });
        save_note_images(note, path, &escaped_title, &image_options)?;
    }
    let content =
        editor::enml_to_text_with_options(&note.content, format.text_format(), &image_options);
    let output_path = path.join(format!("{}{}", escaped_title, format.extension()));
    fs::write(&output_path, remove_control_characters(&content))?;
    Ok(output_path)
}

pub fn escape_path_component(value: &str) -> String {
    value.replace(std::path::MAIN_SEPARATOR, "-")
}

fn save_note_images(
    note: &Note,
    path: &Path,
    escaped_title: &str,
    image_options: &ImageOptions,
) -> Result<()> {
    let image_dir = if image_options.images_in_subdir {
        let image_dir = path.join(format!("{escaped_title}_images"));
        fs::create_dir_all(&image_dir)?;
        image_dir
    } else {
        path.to_path_buf()
    };

    for resource in note
        .resources
        .iter()
        .filter(|resource| resource.mime.as_deref().and_then(image_extension).is_some())
    {
        if !resource.data.body.is_empty() {
            let hash = resource_hash(resource);
            let extension = resource
                .mime
                .as_deref()
                .and_then(image_extension)
                .unwrap_or("bin");
            let filename = format!("{escaped_title}-{hash}.{extension}");
            fs::write(image_dir.join(filename), &resource.data.body)?;
        }
    }
    Ok(())
}

fn resource_hash(resource: &Resource) -> String {
    if !resource.data.body_hash.is_empty() {
        return resource.data.body_hash.clone();
    }
    format!("{:x}", md5::compute(&resource.data.body))
}

fn image_extension(mime: &str) -> Option<&str> {
    match mime.strip_prefix("image/")? {
        "svg+xml" => Some("svg"),
        "jpeg" => Some("jpg"),
        extension => Some(extension),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_control_characters() {
        assert_eq!(
            remove_control_characters("\0This is an english\x01 sentence."),
            "This is an english sentence."
        );
        assert_eq!(
            remove_control_characters("한국\x02어입니\x03다"),
            "한국어입니다"
        );
    }

    #[test]
    fn parses_front_matter() {
        let parsed = parse_meta("---\ntitle: Hello\ntags: [a, b]\n---\nBody");
        assert_eq!(parsed["title"], "Hello");
        assert_eq!(parsed["content"], "\nBody");
    }

    #[test]
    fn creates_file_with_non_ascii_content() {
        let dir = std::env::temp_dir().join(format!("reeknote-rnsync-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let note = Note {
            title: "Test Note".to_string(),
            content: editor::text_to_enml("œ ž © µ ¶ å õ ý þ ß Ü"),
            ..Note::default()
        };
        let path = create_file_from_note(&note, &dir, SyncFormat::Plain, &ImageOptions::default())
            .unwrap();
        let content = std::fs::read_to_string(path).unwrap();
        assert!(content.contains("œ ž © µ ¶ å õ ý þ ß Ü"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn saves_image_resources_next_to_exported_note() {
        let dir =
            std::env::temp_dir().join(format!("reeknote-rnsync-images-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let body = b"fake png".to_vec();
        let hash = format!("{:x}", md5::compute(&body));
        let note = Note {
            title: "Image Note".to_string(),
            content: editor::wrap_enml(&format!("<en-media type=\"image/png\" hash=\"{hash}\" />")),
            resources: vec![Resource {
                guid: String::new(),
                mime: Some("image/png".to_string()),
                filename: "image.png".to_string(),
                data: crate::models::ResourceData {
                    body_hash: hash.clone(),
                    body: body.clone(),
                    size: body.len(),
                },
            }],
            ..Note::default()
        };
        let image_options = ImageOptions {
            save_images: true,
            ..ImageOptions::default()
        };
        let path = create_file_from_note(&note, &dir, SyncFormat::Html, &image_options).unwrap();
        let content = std::fs::read_to_string(path).unwrap();
        assert!(content.contains(&format!("Image Note-{hash}.png")));
        assert_eq!(
            std::fs::read(dir.join(format!("Image Note-{hash}.png"))).unwrap(),
            body
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
