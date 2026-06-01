use crate::errors::{ReeknoteError, Result};
use crate::models::Resource;
use base64::Engine;
use std::fs;
use std::io::{Cursor, Write};
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

const HORIZONTAL_RULE: &str = "------------------------";

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
    enml_to_text_with_resources(content_enml, &[])
}

pub fn enml_to_text_with_resources(content_enml: &str, resources: &[Resource]) -> String {
    enml_to_text_internal(
        content_enml,
        TextFormat::Markdown,
        &ImageOptions::default(),
        resources,
        false,
        false,
    )
}

pub fn enml_to_terminal_text(content_enml: &str) -> String {
    enml_to_terminal_text_with_resources(content_enml, &[])
}

pub fn enml_to_terminal_text_with_resources(content_enml: &str, resources: &[Resource]) -> String {
    enml_to_terminal_text_with_options(content_enml, resources, false)
}

pub fn enml_to_terminal_text_with_options(
    content_enml: &str,
    resources: &[Resource],
    render_images: bool,
) -> String {
    enml_to_text_internal(
        content_enml,
        TextFormat::Markdown,
        &ImageOptions::default(),
        resources,
        true,
        render_images,
    )
}

pub fn enml_to_text_with_options(
    content_enml: &str,
    format: TextFormat,
    image_options: &ImageOptions,
) -> String {
    enml_to_text_internal(content_enml, format, image_options, &[], false, false)
}

fn enml_to_text_internal(
    content_enml: &str,
    format: TextFormat,
    image_options: &ImageOptions,
    resources: &[Resource],
    terminal_styles: bool,
    render_images: bool,
) -> String {
    let mut body = en_note_body(content_enml)
        .unwrap_or(content_enml)
        .to_string();

    if format == TextFormat::Pre {
        if let Some(pre) = extract_tag_body(&body, "pre") {
            return html_unescape(&pre);
        }
    }

    if image_options.save_images {
        body = replace_media_with_images(&body, image_options, format == TextFormat::Html);
    } else if render_images && format != TextFormat::Html {
        body = replace_media_with_terminal_images(&body, resources);
    } else if format != TextFormat::Html {
        body = replace_media_with_placeholders(&body, resources);
    }

    if format == TextFormat::Html {
        return body;
    }

    body = replace_code_blocks(&body, terminal_styles);
    body = replace_inline_code(&body, terminal_styles);
    body = replace_quote_blocks(&body, terminal_styles);
    body = replace_italic_text(&body, terminal_styles);
    body = replace_bold_text(&body, terminal_styles);
    body = replace_links(&body, terminal_styles);
    body = convert_todos_to_markdown(&body);
    body = replace_simple_tag(&body, "h1", |inner| format!("# {}\n\n", inner.trim()));
    body = replace_simple_tag(&body, "h2", |inner| format!("## {}\n\n", inner.trim()));
    body = replace_simple_tag(&body, "h3", |inner| format!("### {}\n\n", inner.trim()));
    body = replace_paragraphs(&body);
    body = replace_divs(&body);
    body = body
        .replace("<br/>", "\n")
        .replace("<br />", "\n")
        .replace("<br>", "\n");
    body = replace_horizontal_rules(&body);
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

        let Some(media_type) = attr_value(tag, "type") else {
            continue;
        };
        let Some(hash) = attr_value(tag, "hash") else {
            continue;
        };
        let Some(extension) = media_type.strip_prefix("image/") else {
            continue;
        };
        images.push(ImageInfo {
            hash,
            extension: extension.to_string(),
        });
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

#[derive(Clone, Debug, PartialEq, Eq)]
enum MarkdownBlock {
    Text(String),
    Code {
        language: Option<String>,
        code: String,
    },
}

fn markdown_to_html(content: &str, rawmd: bool) -> String {
    let blocks = markdown_blocks(content);

    if !blocks.is_empty()
        && blocks.iter().all(|block| match block {
            MarkdownBlock::Text(text) => parse_task(&markdown_text(text, rawmd)).is_some(),
            MarkdownBlock::Code { .. } => false,
        })
    {
        let mut output = String::new();
        for block in blocks {
            let MarkdownBlock::Text(text) = block else {
                continue;
            };
            let text = markdown_text(&text, rawmd);
            let (checked, text) = parse_task(&text).expect("checked above");
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

    let has_code = blocks
        .iter()
        .any(|block| matches!(block, MarkdownBlock::Code { .. }));
    let paragraph_tag = if has_code { "div" } else { "p" };
    let block_suffix = if has_code { "" } else { "\n" };
    let mut output = String::new();
    for block in blocks {
        match block {
            MarkdownBlock::Text(text) => {
                write_markdown_text_block(&mut output, &text, rawmd, paragraph_tag, block_suffix)
            }
            MarkdownBlock::Code { language, code } => {
                output.push_str(&markdown_code_block_html(
                    &code,
                    language.as_deref(),
                    block_suffix,
                ));
            }
        }
    }
    if has_code {
        output.push_str("<div><br/></div>");
    }
    output
}

fn markdown_blocks(content: &str) -> Vec<MarkdownBlock> {
    let mut blocks = Vec::new();
    let mut text_lines = Vec::new();
    let mut code_lines = Vec::new();
    let mut code_fence = None;

    for line in content.lines() {
        if let Some((fence_len, _)) = code_fence.as_ref() {
            if is_code_fence_close(line, *fence_len) {
                let (_, language) = code_fence.take().expect("checked above");
                blocks.push(MarkdownBlock::Code {
                    language,
                    code: code_lines.join("\n"),
                });
                code_lines.clear();
            } else {
                code_lines.push(line.to_string());
            }
            continue;
        }

        if let Some((fence_len, language)) = code_fence_start(line) {
            push_markdown_text_blocks(&mut blocks, &text_lines.join("\n"));
            text_lines.clear();
            code_fence = Some((fence_len, language));
        } else {
            text_lines.push(line.to_string());
        }
    }

    if let Some((_, language)) = code_fence {
        blocks.push(MarkdownBlock::Code {
            language,
            code: code_lines.join("\n"),
        });
    } else {
        push_markdown_text_blocks(&mut blocks, &text_lines.join("\n"));
    }

    blocks
}

fn push_markdown_text_blocks(blocks: &mut Vec<MarkdownBlock>, content: &str) {
    blocks.extend(
        content
            .split("\n\n")
            .map(|block| block.trim_matches('\n'))
            .filter(|block| !block.trim().is_empty())
            .map(|block| MarkdownBlock::Text(block.to_string())),
    );
}

fn code_fence_start(line: &str) -> Option<(usize, Option<String>)> {
    let trimmed = line.trim_start();
    let fence_len = trimmed
        .as_bytes()
        .iter()
        .take_while(|byte| **byte == b'`')
        .count();
    if fence_len < 3 {
        return None;
    }

    let language = trimmed[fence_len..]
        .split_whitespace()
        .next()
        .and_then(markdown_code_language);
    Some((fence_len, language))
}

fn is_code_fence_close(line: &str, opening_fence_len: usize) -> bool {
    let trimmed = line.trim();
    let fence_len = trimmed
        .as_bytes()
        .iter()
        .take_while(|byte| **byte == b'`')
        .count();
    fence_len >= opening_fence_len && trimmed[fence_len..].trim().is_empty()
}

fn markdown_text(text: &str, rawmd: bool) -> String {
    if rawmd {
        text.to_string()
    } else {
        html_escape_tag(text)
    }
}

fn markdown_code_block_html(code: &str, language: Option<&str>, block_suffix: &str) -> String {
    let language_style = if let Some(language) = language {
        format!(" --en-syntaxLanguage:{language};")
    } else {
        String::new()
    };
    format!(
        "<div style=\"--en-codeblock:true;{language_style} --en-lineWrapping:false;box-sizing: border-box; padding: 8px; font-family: &quot;Fira Code&quot;,&quot;Consolas&quot;,&quot;Monaco&quot;,&quot;Andale Mono&quot;,&quot;Ubuntu Mono&quot;,&quot;Courier New&quot;,monospace font-size: 12px; color: rgb(51, 51, 51); border-top-left-radius: 4px; border-top-right-radius: 4px; border-bottom-right-radius: 4px; border-bottom-left-radius: 4px; background-color: rgb(251, 250, 248); border: 1px solid rgba(0, 0, 0, 0.14902); background-position: initial initial; background-repeat: initial initial;\">{}</div>{block_suffix}",
        markdown_code_lines_html(code)
    )
}

fn markdown_code_lines_html(code: &str) -> String {
    let mut output = String::new();
    for line in code.lines() {
        if line.is_empty() {
            output.push_str("<div><br/></div>");
        } else {
            output.push_str(&format!("<div>{}</div>", html_escape_tag(line)));
        }
    }
    if code.is_empty() {
        output.push_str("<div><br/></div>");
    }
    output
}

fn markdown_code_language(language: &str) -> Option<String> {
    let language = language
        .trim()
        .trim_matches(|character| matches!(character, '"' | '\'' | '{' | '}' | '.'))
        .to_ascii_lowercase();
    let language = match language.as_str() {
        "sh" | "shell" | "zsh" => "bash",
        "py" | "python3" => "python",
        "js" | "node" | "nodejs" => "javascript",
        "ts" => "typescript",
        "c++" => "cpp",
        "c#" => "csharp",
        "ps1" | "pwsh" => "powershell",
        "plain" | "text" => "plaintext",
        "" => return None,
        language => language,
    };

    Some(language.to_string())
}

fn write_markdown_text_block(
    output: &mut String,
    block: &str,
    rawmd: bool,
    paragraph_tag: &str,
    block_suffix: &str,
) {
    let block = markdown_text(block, rawmd);
    let trimmed_start = block.trim_start();
    if let Some(text) = trimmed_start.strip_prefix("### ") {
        output.push_str(&format!("<h3>{}</h3>{block_suffix}", text.trim()));
    } else if let Some(text) = trimmed_start.strip_prefix("## ") {
        output.push_str(&format!("<h2>{}</h2>{block_suffix}", text.trim()));
    } else if let Some(text) = trimmed_start.strip_prefix("# ") {
        output.push_str(&format!("<h1>{}</h1>{block_suffix}", text.trim()));
    } else if trimmed_start.starts_with("**")
        && trimmed_start.ends_with("**")
        && trimmed_start.len() >= 4
    {
        output.push_str(&format!(
            "<{paragraph_tag}><strong>{}</strong></{paragraph_tag}>{block_suffix}",
            &trimmed_start[2..trimmed_start.len() - 2]
        ));
    } else if trimmed_start.starts_with('_')
        && trimmed_start.ends_with('_')
        && trimmed_start.len() >= 2
    {
        output.push_str(&format!(
            "<{paragraph_tag}><em>{}</em></{paragraph_tag}>{block_suffix}",
            &trimmed_start[1..trimmed_start.len() - 1]
        ));
    } else {
        output.push_str(&format!(
            "<{paragraph_tag}>{}</{paragraph_tag}>{block_suffix}",
            trimmed_start.replace('\n', "  \n")
        ));
    }
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
        } else if is_compact_div_text(text) {
            format!("{text}\n")
        } else {
            format!("{text}\n\n")
        }
    })
}

fn is_compact_div_text(text: &str) -> bool {
    text.starts_with("* [ ]") || text.starts_with("* [x]")
}

fn replace_horizontal_rules(content: &str) -> String {
    let separator = format!("\n{HORIZONTAL_RULE}\n\n");
    content.replace("<hr/>", &separator)
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
            let code = html_escape_tag(code.trim());
            if highlight_code {
                format!("\x1b[38;5;81m{code}\x1b[0m")
            } else {
                format!("`{code}`")
            }
        },
    )
}

fn replace_quote_blocks(content: &str, terminal_styles: bool) -> String {
    let content = replace_tag_blocks(
        content,
        "blockquote",
        |_| true,
        |inner| format_quote_block(inner, terminal_styles),
    );
    replace_tag_blocks(&content, "div", is_evernote_quote_tag, |inner| {
        format_quote_block(inner, terminal_styles)
    })
}

fn replace_italic_text(content: &str, terminal_styles: bool) -> String {
    let content = replace_tag_blocks(
        content,
        "em",
        |_| true,
        |inner| format_italic_text(inner, terminal_styles),
    );
    let content = replace_tag_blocks(
        &content,
        "i",
        |_| true,
        |inner| format_italic_text(inner, terminal_styles),
    );
    replace_tag_blocks(&content, "span", is_italic_tag, |inner| {
        format_italic_text(inner, terminal_styles)
    })
}

fn replace_bold_text(content: &str, terminal_styles: bool) -> String {
    let content = replace_tag_blocks(
        content,
        "strong",
        |_| true,
        |inner| format_bold_text(inner, terminal_styles),
    );
    let content = replace_tag_blocks(
        &content,
        "b",
        |_| true,
        |inner| format_bold_text(inner, terminal_styles),
    );
    replace_tag_blocks(&content, "span", is_bold_tag, |inner| {
        format_bold_text(inner, terminal_styles)
    })
}

fn replace_links(content: &str, terminal_styles: bool) -> String {
    replace_tag_blocks_with_open_tag(
        content,
        "a",
        |_| true,
        |open_tag, inner| format_link(open_tag, inner, terminal_styles),
    )
}

fn is_evernote_codeblock_tag(open_tag: &str) -> bool {
    let open_tag = open_tag.to_ascii_lowercase();
    open_tag.contains("-en-codeblock")
        || (open_tag.contains("font-family")
            && open_tag.contains("monospace")
            && open_tag.contains("background-color"))
}

fn is_evernote_quote_tag(open_tag: &str) -> bool {
    let open_tag = open_tag.to_ascii_lowercase();
    if open_tag.contains("-en-codeblock") {
        return false;
    }
    open_tag.contains("border-left")
        && (open_tag.contains("padding-left") || open_tag.contains("margin-left"))
}

fn is_italic_tag(open_tag: &str) -> bool {
    let open_tag = open_tag.to_ascii_lowercase().replace(' ', "");
    open_tag.contains("font-style:italic")
}

fn is_bold_tag(open_tag: &str) -> bool {
    let open_tag = open_tag.to_ascii_lowercase().replace(' ', "");
    [
        "font-weight:bold",
        "font-weight:bolder",
        "font-weight:600",
        "font-weight:700",
        "font-weight:800",
        "font-weight:900",
    ]
    .iter()
    .any(|needle| open_tag.contains(needle))
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

    let code = html_escape_tag(code);
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
        output.push_str(&html_escape_tag(line));
        output.push_str(&" ".repeat(padding));
        output.push_str("\x1b[0m\n");
    }
    output.push('\n');
    output
}

fn format_quote_block(inner: &str, terminal_styles: bool) -> String {
    let quote = code_text_from_html(inner);
    let quote = quote.trim_matches('\n');
    if quote.trim().is_empty() {
        return String::new();
    }

    if terminal_styles {
        highlighted_quote_block(quote)
    } else {
        markdown_quote_block(quote)
    }
}

fn markdown_quote_block(quote: &str) -> String {
    let mut output = String::new();
    for line in quote.lines() {
        if line.trim().is_empty() {
            output.push_str(">\n");
        } else {
            output.push_str("> ");
            output.push_str(&html_escape_tag(line));
            output.push('\n');
        }
    }
    output.push('\n');
    output
}

fn highlighted_quote_block(quote: &str) -> String {
    let mut output = String::new();
    for line in quote.lines() {
        output.push_str("\x1b[38;5;39m|\x1b[0m ");
        if line.trim().is_empty() {
            output.push('\n');
        } else {
            output.push_str("\x1b[3;38;5;245m");
            output.push_str(&html_escape_tag(line));
            output.push_str("\x1b[0m\n");
        }
    }
    output.push('\n');
    output
}

fn format_italic_text(inner: &str, terminal_styles: bool) -> String {
    let text = code_text_from_html(inner);
    let text = text.trim();
    if text.is_empty() {
        return String::new();
    }

    let text = html_escape_tag(text);
    if terminal_styles {
        format!("\x1b[3m{text}\x1b[0m")
    } else {
        format!("_{text}_")
    }
}

fn format_bold_text(inner: &str, terminal_styles: bool) -> String {
    let text = code_text_from_html(inner);
    let text = text.trim();
    if text.is_empty() {
        return String::new();
    }

    let text = html_escape_tag(text);
    if terminal_styles {
        format!("\x1b[1m{text}\x1b[0m")
    } else {
        format!("**{text}**")
    }
}

fn format_link(open_tag: &str, inner: &str, terminal_styles: bool) -> String {
    let label = html_escape_tag(code_text_from_html(inner).trim());
    let href = attr_value(open_tag, "href").map(|href| html_escape_tag(&html_unescape(&href)));
    let link = match (label.is_empty(), href) {
        (true, Some(href)) => href,
        (false, Some(href)) if terminal_styles && label == href => href,
        (false, Some(href)) => format!("[{label}]({href})"),
        (false, None) => return label,
        (true, None) => return String::new(),
    };

    if terminal_styles {
        blue_terminal_text(&link)
    } else {
        link
    }
}

fn blue_terminal_text(text: &str) -> String {
    let text = text.replace("\x1b[0m", "\x1b[0m\x1b[34m");
    format!("\x1b[34m{text}\x1b[0m")
}

fn italic_terminal_text(text: &str) -> String {
    format!("\x1b[3m{text}\x1b[0m")
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
    replace_tag_blocks_with_open_tag(content, tag, predicate, |_, inner| formatter(inner))
}

fn replace_tag_blocks_with_open_tag<P, F>(
    content: &str,
    tag: &str,
    predicate: P,
    formatter: F,
) -> String
where
    P: Fn(&str) -> bool,
    F: Fn(&str, &str) -> String,
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
            output.push_str(&formatter(open_tag, &rest[body_start..body_end]));
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

        if let (Some(media_type), Some(hash), Some(base_filename)) = (
            attr_value(tag, "type"),
            attr_value(tag, "hash"),
            image_options.base_filename.as_ref(),
        ) {
            if let Some(extension) = media_type.strip_prefix("image/") {
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
        }
        output.push_str("<en-media");
        output.push_str(tag);
        output.push('>');
    }
    output.push_str(rest);
    output
}

fn replace_media_with_terminal_images(content: &str, resources: &[Resource]) -> String {
    let mut output = String::new();
    let mut rest = content;
    let mut image_index = 0usize;

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

        if let Some(image) = terminal_image_for_media(tag, resources, image_index) {
            let filename = media_filename(
                attr_value(tag, "type").as_deref(),
                attr_value(tag, "hash").as_deref(),
                resources,
            );
            image_index += 1;
            output.push_str(&image);
            output.push('\n');
            output.push_str(&italic_terminal_text(&filename));
            output.push_str("\n\n");
        } else {
            output.push_str(&format!("{}\n\n", media_placeholder(tag, resources)));
        }
    }
    output.push_str(rest);
    output
}

fn terminal_image_for_media(
    tag: &str,
    resources: &[Resource],
    image_index: usize,
) -> Option<String> {
    let media_type = attr_value(tag, "type");
    if !media_type
        .as_deref()
        .is_some_and(|value| value.starts_with("image/"))
    {
        return None;
    }
    let hash = attr_value(tag, "hash")?;
    let resource = resources
        .iter()
        .find(|resource| resource.data.body_hash == hash)?;
    if resource.data.body.is_empty() {
        return None;
    }
    let path = write_terminal_image_temp_file(resource, image_index).ok()?;
    Some(kitty_image_command(&path))
}

fn write_terminal_image_temp_file(resource: &Resource, image_index: usize) -> Result<PathBuf> {
    let image = image::load_from_memory(&resource.data.body)
        .map_err(|error| ReeknoteError::External(format!("cannot decode image: {error}")))?;
    let mut png = Cursor::new(Vec::new());
    image
        .write_to(&mut png, image::ImageFormat::Png)
        .map_err(|error| ReeknoteError::External(format!("cannot encode image as PNG: {error}")))?;
    let mut file = tempfile::Builder::new()
        .prefix(&format!("reeknote-image-{image_index}-"))
        .suffix(".png")
        .tempfile()?;
    file.write_all(png.get_ref())?;
    let (_, path) = file.keep().map_err(|error| error.error)?;
    Ok(path)
}

fn kitty_image_command(path: &Path) -> String {
    format!(
        "\x1b_Ga=T,t=f,f=100,q=2;{}\x1b\\",
        base64::engine::general_purpose::STANDARD.encode(path.to_string_lossy().as_bytes())
    )
}

fn replace_media_with_placeholders(content: &str, resources: &[Resource]) -> String {
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

        output.push_str(&format!("{}\n\n", media_placeholder(tag, resources)));
    }
    output.push_str(rest);
    output
}

fn media_placeholder(tag: &str, resources: &[Resource]) -> String {
    let media_type = attr_value(tag, "type");
    let hash = attr_value(tag, "hash");
    let label = if media_type
        .as_deref()
        .is_some_and(|value| value.starts_with("image/"))
    {
        "Image"
    } else {
        "Attachment"
    };
    let filename = media_filename(media_type.as_deref(), hash.as_deref(), resources);
    let prefix = if media_type
        .as_deref()
        .is_some_and(|value| value.to_ascii_lowercase().starts_with("audio/"))
    {
        "🎧 "
    } else {
        ""
    };
    format!("{prefix}[{label}: {filename}]")
}

fn media_filename(media_type: Option<&str>, hash: Option<&str>, resources: &[Resource]) -> String {
    if let Some(filename) = hash
        .and_then(|hash| {
            resources
                .iter()
                .find(|resource| resource.data.body_hash == hash)
        })
        .and_then(|resource| {
            if resource.filename.is_empty() {
                None
            } else {
                Some(resource.filename.clone())
            }
        })
    {
        return filename;
    }

    let prefix = if media_type
        .as_ref()
        .is_some_and(|value| value.starts_with("image/"))
    {
        "image"
    } else {
        "attachment"
    };
    match (hash, media_type.and_then(media_extension)) {
        (Some(hash), Some(extension)) => format!("{prefix}-{hash}.{extension}"),
        (Some(hash), None) => format!("{prefix}-{hash}"),
        (None, Some(extension)) => format!("{prefix}.{extension}"),
        (None, None) => prefix.to_string(),
    }
}

fn media_extension(media_type: &str) -> Option<&str> {
    match media_type {
        "image/jpeg" => Some("jpg"),
        "image/svg+xml" => Some("svg"),
        value => value.strip_prefix("image/"),
    }
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
            '>' if in_tag => in_tag = false,
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
