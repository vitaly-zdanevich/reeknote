use crate::errors::{ReeknoteError, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextFormat {
    Markdown,
    Html,
    Plain,
    Pre,
}

impl TextFormat {
    pub fn from_extension(extension: &str) -> Option<Self> {
        match extension {
            ".md" | ".markdown" => Some(Self::Markdown),
            ".html" | ".org" => Some(Self::Html),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ImageInfo {
    pub hash: String,
    pub extension: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ImageOptions {
    pub save_images: bool,
    pub images_in_subdir: bool,
    pub base_filename: Option<String>,
}

pub fn html_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
        .replace('\n', "<br />")
}

pub fn html_escape_tag(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

pub fn html_unescape(text: &str) -> String {
    text.replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&nbsp;", " ")
}

pub fn wrap_enml(content_html: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE en-note SYSTEM \"http://xml.evernote.com/pub/enml2.dtd\">\n<en-note>{content_html}</en-note>"
    )
}

pub fn text_to_enml(content: &str) -> String {
    text_to_enml_with_options(content, TextFormat::Markdown, false)
}

pub fn text_to_enml_with_options(content: &str, format: TextFormat, rawmd: bool) -> String {
    let content_html = match format {
        TextFormat::Pre => format!("<pre>{}</pre>", html_escape(content)),
        TextFormat::Markdown => markdown_to_html(content, rawmd),
        TextFormat::Html => sanitize_html(content),
        TextFormat::Plain => plain_to_html(content),
    };
    wrap_enml(&content_html)
}

pub fn enml_to_text(content_enml: &str) -> String {
    enml_to_text_internal(
        content_enml,
        TextFormat::Markdown,
        &ImageOptions::default(),
        false,
    )
}

pub fn enml_to_terminal_text(content_enml: &str) -> String {
    enml_to_text_internal(
        content_enml,
        TextFormat::Markdown,
        &ImageOptions::default(),
        true,
    )
}

pub fn enml_to_text_with_options(
    content_enml: &str,
    format: TextFormat,
    image_options: &ImageOptions,
) -> String {
    enml_to_text_internal(content_enml, format, image_options, false)
}

fn enml_to_text_internal(
    content_enml: &str,
    format: TextFormat,
    image_options: &ImageOptions,
    highlight_code: bool,
) -> String {
    let mut body = en_note_body(content_enml)
        .unwrap_or(content_enml)
        .to_string();

    if format == TextFormat::Pre
        && let Some(pre) = extract_tag_body(&body, "pre")
    {
        return html_unescape(&pre);
    }

    if image_options.save_images {
        body = replace_media_with_images(&body, image_options, format == TextFormat::Html);
    }

    if format == TextFormat::Html {
        return body;
    }

    body = replace_code_blocks(&body, highlight_code);
    body = replace_inline_code(&body, highlight_code);
    body = convert_todos_to_markdown(&body);
    body = replace_simple_tag(&body, "h1", |inner| {
        format!("# {}\n\n", html_unescape(inner).trim())
    });
    body = replace_simple_tag(&body, "h2", |inner| {
        format!("## {}\n\n", html_unescape(inner).trim())
    });
    body = replace_simple_tag(&body, "h3", |inner| {
        format!("### {}\n\n", html_unescape(inner).trim())
    });
    body = replace_simple_tag(&body, "strong", |inner| {
        format!("**{}**", html_unescape(inner).trim())
    });
    body = replace_simple_tag(&body, "em", |inner| {
        format!("_{}_", html_unescape(inner).trim())
    });
    body = replace_paragraphs(&body);
    body = replace_divs(&body);
    body = body
        .replace("<br/>", "\n")
        .replace("<br />", "\n")
        .replace("<br>", "\n");
    body = strip_tags(&body);
    normalize_blank_lines(&html_unescape(&body))
}

pub fn get_images(content_enml: &str) -> Vec<ImageInfo> {
    let mut images = Vec::new();
    let mut rest = content_enml;

    while let Some(index) = rest.find("<en-media") {
        rest = &rest[index + "<en-media".len()..];
        let Some(end) = rest.find('>') else {
            break;
        };
        let tag = &rest[..end];
        rest = &rest[end + 1..];

        let media_type = attr_value(tag, "type");
        let hash = attr_value(tag, "hash");
        if let (Some(media_type), Some(hash)) = (media_type, hash)
            && let Some(extension) = media_type.strip_prefix("image/")
        {
            images.push(ImageInfo {
                hash,
                extension: extension.to_string(),
            });
        }
    }

    images
}

pub fn read_edit_result(original_checksum: &str, new_checksum: &str) -> bool {
    original_checksum != new_checksum
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditOutcome {
    pub content: String,
    pub changed: bool,
    pub path: PathBuf,
}

pub fn edit_content(
    editor_command: &str,
    initial_content: &str,
    suffix: &str,
) -> Result<EditOutcome> {
    let path = temp_note_path(suffix);
    fs::write(&path, initial_content)?;
    let before = fs::read(&path)?;
    run_editor(editor_command, &path)?;
    let after = fs::read(&path)?;
    let content = String::from_utf8(after.clone())
        .map_err(|_| ReeknoteError::InvalidInput("edited note is not valid UTF-8".to_string()))?;
    let changed = before != after;
    let _ = fs::remove_file(&path);
    Ok(EditOutcome {
        content,
        changed,
        path,
    })
}

fn temp_note_path(suffix: &str) -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let suffix = if suffix.starts_with('.') {
        suffix.to_string()
    } else {
        format!(".{suffix}")
    };
    std::env::temp_dir().join(format!(
        "reeknote-{}-{timestamp}{suffix}",
        std::process::id()
    ))
}

fn run_editor(editor_command: &str, path: &Path) -> Result<()> {
    let command = format!("{} {}", editor_command, shell_quote(path));
    let status = Command::new("sh").arg("-c").arg(command).status()?;
    if status.success() {
        Ok(())
    } else {
        Err(ReeknoteError::External(format!(
            "editor exited with status {status}"
        )))
    }
}

fn shell_quote(path: &Path) -> String {
    let value = path.to_string_lossy();
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn markdown_to_html(content: &str, rawmd: bool) -> String {
    let source = if rawmd {
        content.to_string()
    } else {
        html_escape_tag(content)
    };

    let blocks: Vec<&str> = source.split("\n\n").collect();
    let non_empty: Vec<&str> = blocks
        .into_iter()
        .map(|block| block.trim_matches('\n'))
        .filter(|block| !block.trim().is_empty())
        .collect();

    if !non_empty.is_empty() && non_empty.iter().all(|block| parse_task(block).is_some()) {
        let mut output = String::new();
        for block in non_empty {
            let (checked, text) = parse_task(block).expect("checked above");
            if checked {
                output.push_str(&format!(
                    "<div><en-todo checked=\"true\"></en-todo>{}</div>",
                    text.trim()
                ));
            } else {
                output.push_str(&format!("<div><en-todo></en-todo>{}</div>", text.trim()));
            }
        }
        output.push('\n');
        return output;
    }

    let mut output = String::new();
    for block in non_empty {
        let trimmed_start = block.trim_start();
        if let Some(text) = trimmed_start.strip_prefix("### ") {
            output.push_str(&format!("<h3>{}</h3>\n", text.trim()));
        } else if let Some(text) = trimmed_start.strip_prefix("## ") {
            output.push_str(&format!("<h2>{}</h2>\n", text.trim()));
        } else if let Some(text) = trimmed_start.strip_prefix("# ") {
            output.push_str(&format!("<h1>{}</h1>\n", text.trim()));
        } else if trimmed_start.starts_with("**")
            && trimmed_start.ends_with("**")
            && trimmed_start.len() >= 4
        {
            output.push_str(&format!(
                "<p><strong>{}</strong></p>\n",
                &trimmed_start[2..trimmed_start.len() - 2]
            ));
        } else if trimmed_start.starts_with('_')
            && trimmed_start.ends_with('_')
            && trimmed_start.len() >= 2
        {
            output.push_str(&format!(
                "<p><em>{}</em></p>\n",
                &trimmed_start[1..trimmed_start.len() - 1]
            ));
        } else {
            output.push_str(&format!("<p>{}</p>\n", trimmed_start.replace('\n', "  \n")));
        }
    }
    output
}

fn parse_task(block: &str) -> Option<(bool, &str)> {
    let block = block.trim();
    if let Some(rest) = block.strip_prefix("* [ ]") {
        Some((false, rest))
    } else {
        block.strip_prefix("* [x]").map(|rest| (true, rest))
    }
}

fn sanitize_html(content: &str) -> String {
    let mut output = String::with_capacity(content.len());
    let mut rest = content;

    while let Some(start) = rest.find('<') {
        output.push_str(&rest[..start]);
        rest = &rest[start..];
        let Some(end) = rest.find('>') else {
            output.push_str(rest);
            return output;
        };
        let tag = &rest[..=end];
        output.push_str(&sanitize_tag(tag));
        rest = &rest[end + 1..];
    }
    output.push_str(rest);
    output
}

fn sanitize_tag(tag: &str) -> String {
    if tag.starts_with("</") || tag.starts_with("<!") || tag.starts_with("<?") {
        return tag.to_string();
    }

    let inner = tag.trim_start_matches('<').trim_end_matches('>');
    let self_close = inner.ends_with('/');
    let inner = inner.trim_end_matches('/').trim();
    let mut parts = inner.split_whitespace();
    let Some(name) = parts.next() else {
        return tag.to_string();
    };

    let mut attrs = Vec::new();
    for attr in parts {
        let key = attr.split('=').next().unwrap_or(attr);
        if matches!(
            key,
            "id" | "class" | "accesskey" | "data" | "dynsrc" | "tabindex"
        ) || key.starts_with("on")
        {
            continue;
        }
        attrs.push(attr);
    }

    let mut output = String::from('<');
    output.push_str(name);
    for attr in attrs {
        output.push(' ');
        output.push_str(attr);
    }
    if self_close {
        output.push_str(" /");
    }
    output.push('>');
    output
}

fn plain_to_html(content: &str) -> String {
    let escaped = html_escape(content);
    let mut output = String::new();
    for line in escaped.split("<br />") {
        if line.is_empty() {
            output.push_str("<div><br/></div>");
        } else {
            let line = line
                .replace("[x]", "<en-todo checked=\"true\"></en-todo>")
                .replace("[ ]", "<en-todo></en-todo>");
            output.push_str(&format!("<div>{line}</div>"));
        }
    }
    output
}

fn en_note_body(content: &str) -> Option<&str> {
    let start = content.find("<en-note>")? + "<en-note>".len();
    let end = content.rfind("</en-note>")?;
    Some(&content[start..end])
}

fn extract_tag_body(content: &str, tag: &str) -> Option<String> {
    let start_tag = format!("<{tag}>");
    let end_tag = format!("</{tag}>");
    let start = content.find(&start_tag)? + start_tag.len();
    let end = content[start..].find(&end_tag)? + start;
    Some(content[start..end].to_string())
}

fn replace_simple_tag<F>(content: &str, tag: &str, formatter: F) -> String
where
    F: Fn(&str) -> String,
{
    let start_tag = format!("<{tag}>");
    let end_tag = format!("</{tag}>");
    let mut output = String::new();
    let mut rest = content;

    while let Some(start) = rest.find(&start_tag) {
        output.push_str(&rest[..start]);
        let inner_start = start + start_tag.len();
        let Some(relative_end) = rest[inner_start..].find(&end_tag) else {
            output.push_str(&rest[start..]);
            return output;
        };
        let inner_end = inner_start + relative_end;
        output.push_str(&formatter(&rest[inner_start..inner_end]));
        rest = &rest[inner_end + end_tag.len()..];
    }
    output.push_str(rest);
    output
}

fn replace_paragraphs(content: &str) -> String {
    replace_simple_tag(content, "p", |inner| {
        let text = inner.trim();
        if text.is_empty() {
            "\n".to_string()
        } else {
            format!("{}\n\n", text)
        }
    })
}

fn replace_divs(content: &str) -> String {
    replace_simple_tag(content, "div", |inner| {
        let text = inner.trim();
        if text == "<br/>" || text == "<br />" {
            "\n".to_string()
        } else {
            format!("{}\n", text)
        }
    })
}

fn replace_code_blocks(content: &str, highlight_code: bool) -> String {
    let content = replace_tag_blocks(content, "div", is_evernote_codeblock_tag, |inner| {
        format_code_block(inner, highlight_code)
    });
    replace_tag_blocks(
        &content,
        "pre",
        |_| true,
        |inner| format_code_block(inner, highlight_code),
    )
}

fn replace_inline_code(content: &str, highlight_code: bool) -> String {
    replace_tag_blocks(
        content,
        "code",
        |_| true,
        |inner| {
            let code = code_text_from_html(inner);
            if highlight_code {
                format!("\x1b[38;5;81m{}\x1b[0m", code.trim())
            } else {
                format!("`{}`", code.trim())
            }
        },
    )
}

fn is_evernote_codeblock_tag(open_tag: &str) -> bool {
    let open_tag = open_tag.to_ascii_lowercase();
    open_tag.contains("-en-codeblock")
        || (open_tag.contains("font-family")
            && open_tag.contains("monospace")
            && open_tag.contains("background-color"))
}

fn format_code_block(inner: &str, highlight_code: bool) -> String {
    let code = code_text_from_html(inner);
    let code = code.trim_matches('\n');
    if code.trim().is_empty() {
        return String::new();
    }

    if highlight_code {
        return highlighted_code_block(code);
    }

    format!("```\n{code}\n```\n\n")
}

fn highlighted_code_block(code: &str) -> String {
    let width = code
        .lines()
        .map(|line| line.chars().count())
        .max()
        .unwrap_or_default();
    let mut output = String::new();
    for line in code.lines() {
        let padding = width.saturating_sub(line.chars().count()) + 1;
        output.push_str("\x1b[48;5;236;38;5;252m ");
        output.push_str(line);
        output.push_str(&" ".repeat(padding));
        output.push_str("\x1b[0m\n");
    }
    output.push('\n');
    output
}

fn code_text_from_html(content: &str) -> String {
    let content = strip_intertag_whitespace(content);
    let mut output = String::new();
    let mut tag = String::new();
    let mut in_tag = false;

    for character in content.chars() {
        if in_tag {
            tag.push(character);
            if character == '>' {
                append_code_tag_spacing(&mut output, &tag);
                tag.clear();
                in_tag = false;
            }
        } else if character == '<' {
            tag.push(character);
            in_tag = true;
        } else {
            output.push(character);
        }
    }

    if !tag.is_empty() {
        output.push_str(&tag);
    }

    html_unescape(&output)
}

fn append_code_tag_spacing(output: &mut String, tag: &str) {
    let tag = tag.to_ascii_lowercase();
    let tag = tag.trim();
    if tag.starts_with("<br") || tag.starts_with("</div") || tag.starts_with("</p") {
        output.push('\n');
    }
}

fn strip_intertag_whitespace(content: &str) -> String {
    let mut output = String::new();
    let mut rest = content;

    while let Some(end) = rest.find('>') {
        let end = end + 1;
        output.push_str(&rest[..end]);
        rest = &rest[end..];

        let Some(next_start) = rest.find('<') else {
            output.push_str(rest);
            return output;
        };
        let between = &rest[..next_start];
        if !between.trim().is_empty() {
            output.push_str(between);
        }
        rest = &rest[next_start..];
    }

    output.push_str(rest);
    output
}

fn replace_tag_blocks<P, F>(content: &str, tag: &str, predicate: P, formatter: F) -> String
where
    P: Fn(&str) -> bool,
    F: Fn(&str) -> String,
{
    let mut output = String::new();
    let mut rest = content;

    while let Some((open_start, open_end, open_tag)) = find_open_tag(rest, tag) {
        output.push_str(&rest[..open_start]);
        if predicate(open_tag) {
            let body_start = open_end;
            let Some((body_end, close_end)) = find_matching_close_tag(rest, tag, body_start) else {
                output.push_str(&rest[open_start..]);
                return output;
            };
            output.push_str(&formatter(&rest[body_start..body_end]));
            rest = &rest[close_end..];
        } else {
            output.push_str(&rest[open_start..open_end]);
            rest = &rest[open_end..];
        }
    }

    output.push_str(rest);
    output
}

fn find_open_tag<'a>(content: &'a str, tag: &str) -> Option<(usize, usize, &'a str)> {
    let needle = format!("<{tag}");
    let mut offset = 0;
    while let Some(relative_start) = content[offset..].find(&needle) {
        let start = offset + relative_start;
        let name_end = start + needle.len();
        let next = content[name_end..].chars().next();
        if next.is_some_and(|character| matches!(character, '>' | '/' | ' ' | '\t' | '\n')) {
            let relative_end = content[name_end..].find('>')?;
            let end = name_end + relative_end + 1;
            return Some((start, end, &content[start..end]));
        }
        offset = name_end;
    }
    None
}

fn find_matching_close_tag(content: &str, tag: &str, body_start: usize) -> Option<(usize, usize)> {
    let close_needle = format!("</{tag}>");
    let mut depth = 1usize;
    let mut cursor = body_start;

    while cursor < content.len() {
        let next_open = find_open_tag(&content[cursor..], tag)
            .map(|(start, end, open_tag)| (cursor + start, cursor + end, open_tag.ends_with("/>")));
        let next_close = content[cursor..]
            .find(&close_needle)
            .map(|start| cursor + start);

        match (next_open, next_close) {
            (Some((open_start, open_end, self_closing)), Some(close_start))
                if open_start < close_start =>
            {
                if !self_closing {
                    depth += 1;
                }
                cursor = open_end;
            }
            (_, Some(close_start)) => {
                depth -= 1;
                let close_end = close_start + close_needle.len();
                if depth == 0 {
                    return Some((close_start, close_end));
                }
                cursor = close_end;
            }
            (Some((_, open_end, self_closing)), None) => {
                if !self_closing {
                    depth += 1;
                }
                cursor = open_end;
            }
            (None, None) => return None,
        }
    }

    None
}

fn convert_todos_to_markdown(content: &str) -> String {
    let mut output = content.replace("<en-todo checked=\"true\"></en-todo>", "* [x]");
    output = output.replace("<en-todo checked=\"true\"/>", "* [x]");
    output = output.replace("<en-todo checked=\"true\" />", "* [x]");
    output = output.replace("<en-todo></en-todo>", "* [ ]");
    output = output.replace("<en-todo/>", "* [ ]");
    output.replace("<en-todo />", "* [ ]")
}

fn replace_media_with_images(content: &str, image_options: &ImageOptions, html: bool) -> String {
    let mut output = String::new();
    let mut rest = content;

    while let Some(index) = rest.find("<en-media") {
        output.push_str(&rest[..index]);
        rest = &rest[index + "<en-media".len()..];
        let Some(end) = rest.find('>') else {
            output.push_str("<en-media");
            output.push_str(rest);
            return output;
        };
        let tag = &rest[..end];
        rest = &rest[end + 1..];

        let media_type = attr_value(tag, "type");
        let hash = attr_value(tag, "hash");
        if let (Some(media_type), Some(hash), Some(base_filename)) =
            (media_type, hash, image_options.base_filename.as_ref())
            && let Some(extension) = media_type.strip_prefix("image/")
        {
            let extension = match extension {
                "svg+xml" => "svg",
                "jpeg" => "jpg",
                value => value,
            };
            let source = format!("{base_filename}-{hash}.{extension}");
            if html {
                output.push_str(&format!("<img src=\"{source}\">"));
            } else {
                output.push_str(&format!("![image]({source})"));
            }
            continue;
        }
        output.push_str("<en-media");
        output.push_str(tag);
        output.push('>');
    }
    output.push_str(rest);
    output
}

fn attr_value(tag: &str, key: &str) -> Option<String> {
    for quote in ['"', '\''] {
        let needle = format!("{key}={quote}");
        if let Some(start) = tag.find(&needle) {
            let value_start = start + needle.len();
            let value_end = tag[value_start..].find(quote)? + value_start;
            return Some(tag[value_start..value_end].to_string());
        }
    }
    None
}

fn strip_tags(content: &str) -> String {
    let mut output = String::new();
    let mut in_tag = false;
    for character in content.chars() {
        match character {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => output.push(character),
            _ => {}
        }
    }
    output
}

fn normalize_blank_lines(content: &str) -> String {
    let content = content.replace("\r\n", "\n");
    let mut output = String::new();
    let mut previous_blank = false;

    for line in content.lines() {
        let blank = line.trim().is_empty();
        if blank {
            if !previous_blank {
                output.push('\n');
            }
        } else {
            output.push_str(line.trim_end());
            output.push('\n');
        }
        previous_blank = blank;
    }

    if !output.ends_with('\n') {
        output.push('\n');
    }
    output
}

pub fn ensure_string_content(content: Option<&str>) -> Result<&str> {
    content.ok_or_else(|| ReeknoteError::InvalidInput("note content is required".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const MD_TEXT: &str = "# Header 1\n\n## Header 2\n\nLine 1\n\n_Line 2_\n\n**Line 3**\n\n";
    const HTML_TEXT: &str = "<h1>Header 1</h1>\n<h2>Header 2</h2>\n<p>Line 1</p>\n<p><em>Line 2</em></p>\n<p><strong>Line 3</strong></p>\n";

    #[test]
    fn converts_markdown_to_enml() {
        assert_eq!(text_to_enml(MD_TEXT), wrap_enml(HTML_TEXT));
    }

    #[test]
    fn converts_enml_to_markdown() {
        assert_eq!(enml_to_text(&wrap_enml(HTML_TEXT)), MD_TEXT);
    }

    #[test]
    fn converts_task_lists() {
        let markdown = "\n* [ ]item 1\n\n* [x]item 2\n\n* [ ]item 3\n\n";
        let html = "<div><en-todo></en-todo>item 1</div><div><en-todo checked=\"true\"></en-todo>item 2</div><div><en-todo></en-todo>item 3</div>\n";
        assert_eq!(text_to_enml(markdown), wrap_enml(html));
        assert_eq!(
            enml_to_text(&wrap_enml(html)),
            "* [ ]item 1\n* [x]item 2\n* [ ]item 3\n\n"
        );
    }

    #[test]
    fn escapes_markdown_html() {
        assert_eq!(
            text_to_enml("<what ever>"),
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE en-note SYSTEM \"http://xml.evernote.com/pub/enml2.dtd\">\n<en-note><p>&lt;what ever&gt;</p>\n</en-note>"
        );
    }

    #[test]
    fn extracts_images() {
        let images = get_images("<en-note><en-media type=\"image/png\" hash=\"abc\" /></en-note>");
        assert_eq!(
            images,
            vec![ImageInfo {
                hash: "abc".to_string(),
                extension: "png".to_string()
            }]
        );
    }

    #[test]
    fn converts_media_to_local_image_reference() {
        let options = ImageOptions {
            save_images: true,
            base_filename: Some("Note".to_string()),
            ..ImageOptions::default()
        };
        let markdown = enml_to_text_with_options(
            "<en-note><en-media type=\"image/png\" hash=\"abc\" /></en-note>",
            TextFormat::Markdown,
            &options,
        );
        assert!(markdown.contains("![image](Note-abc.png)"));
        let html = enml_to_text_with_options(
            "<en-note><en-media type=\"image/png\" hash=\"abc\" /></en-note>",
            TextFormat::Html,
            &options,
        );
        assert!(html.contains("<img src=\"Note-abc.png\">"));
    }

    #[test]
    fn converts_pre_blocks_to_markdown_code_blocks() {
        let text = enml_to_text(&wrap_enml("<pre>let answer = 42;</pre>"));
        assert_eq!(text, "```\nlet answer = 42;\n```\n\n");
    }

    #[test]
    fn converts_inline_code_to_markdown_code() {
        let text = enml_to_text(&wrap_enml("<div>Run <code>cargo test</code></div>"));
        assert_eq!(text, "Run `cargo test`\n");
    }

    #[test]
    fn converts_evernote_codeblock_divs_to_markdown_code_blocks() {
        let html = r#"<div style="box-sizing: border-box; font-family: Monaco, Menlo, Consolas, &quot;Courier New&quot;, monospace; background-color: rgb(251, 250, 248); -en-codeblock:true;"><div>fn main() {</div><div>    println!(&quot;ok&quot;);</div><div>}</div></div>"#;
        let text = enml_to_text(&wrap_enml(html));
        assert_eq!(text, "```\nfn main() {\n    println!(\"ok\");\n}\n```\n\n");
    }

    #[test]
    fn highlights_code_blocks_for_terminal_output() {
        let text = enml_to_terminal_text(&wrap_enml("<pre>let answer = 42;</pre>"));
        assert!(text.contains("\x1b[48;5;236;38;5;252m let answer = 42; \x1b[0m"));
        assert!(!text.contains("```"));
    }

    #[test]
    fn highlights_inline_code_for_terminal_output() {
        let text = enml_to_terminal_text(&wrap_enml("<div>Run <code>cargo test</code></div>"));
        assert_eq!(text, "Run \x1b[38;5;81mcargo test\x1b[0m\n");
    }

    #[test]
    fn edits_content_with_external_command() {
        let outcome = edit_content("sed -i s/original/edited/", "original", ".md").unwrap();
        assert_eq!(outcome.content, "edited");
        assert!(outcome.changed);
        assert!(!outcome.path.exists());
    }
}
