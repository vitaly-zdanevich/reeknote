use reeknote::editor::{
    ImageInfo, ImageOptions, TextFormat, enml_to_terminal_text, enml_to_text,
    enml_to_text_with_options, enml_to_text_with_resources, get_images, text_to_enml, wrap_enml,
};
use reeknote::models::{Resource, ResourceData};

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
fn converts_div_blocks_to_paragraphs() {
    let html = "<div>First paragraph</div><div>Second paragraph</div>";
    assert_eq!(
        enml_to_text(&wrap_enml(html)),
        "First paragraph\n\nSecond paragraph\n\n"
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
fn converts_images_to_filename_placeholders() {
    let note = wrap_enml(r#"<en-media type="image/png" hash="abc" />"#);
    let resources = vec![resource("abc", "image/png", "photo.png")];
    assert_eq!(
        enml_to_text_with_resources(&note, &resources),
        "[Image: photo.png]\n\n"
    );
}

#[test]
fn converts_images_to_fallback_placeholders() {
    let note = wrap_enml(r#"<en-media type="image/png" hash="abc" />"#);
    assert_eq!(enml_to_text(&note), "[Image: image-abc.png]\n\n");
}

#[test]
fn separates_image_placeholders_from_following_text() {
    let note = wrap_enml(r#"<en-media type="image/jpeg" hash="abc" />blablabla"#);
    assert_eq!(enml_to_text(&note), "[Image: image-abc.jpg]\n\nblablabla\n");
}

#[test]
fn converts_attachments_to_filename_placeholders() {
    let note = wrap_enml(r#"<en-media type="application/pdf" hash="abc" />"#);
    let resources = vec![resource("abc", "application/pdf", "document.pdf")];
    assert_eq!(
        enml_to_text_with_resources(&note, &resources),
        "[Attachment: document.pdf]\n\n"
    );
}

#[test]
fn converts_pre_blocks_to_markdown_code_blocks() {
    let text = enml_to_text(&wrap_enml("<pre>let answer = 42;</pre>"));
    assert_eq!(text, "```\nlet answer = 42;\n```\n\n");
}

#[test]
fn converts_inline_code_to_markdown_code() {
    let text = enml_to_text(&wrap_enml("<div>Run <code>cargo test</code></div>"));
    assert_eq!(text, "Run `cargo test`\n\n");
}

#[test]
fn converts_i_tags_to_markdown_italic() {
    let text = enml_to_text(&wrap_enml("<div>This is <i>important</i></div>"));
    assert_eq!(text, "This is _important_\n\n");
}

#[test]
fn converts_styled_spans_to_markdown_italic() {
    let html = r#"<div>This is <span style="font-style: italic;">important</span></div>"#;
    let text = enml_to_text(&wrap_enml(html));
    assert_eq!(text, "This is _important_\n\n");
}

#[test]
fn highlights_italic_for_terminal_output() {
    let text = enml_to_terminal_text(&wrap_enml("<div>This is <i>important</i></div>"));
    assert_eq!(text, "This is \x1b[3mimportant\x1b[0m\n\n");
}

#[test]
fn converts_b_tags_to_markdown_bold() {
    let text = enml_to_text(&wrap_enml("<div>This is <b>important</b></div>"));
    assert_eq!(text, "This is **important**\n\n");
}

#[test]
fn converts_styled_spans_to_markdown_bold() {
    let html = r#"<div>This is <span style="font-weight: bold;">important</span></div>"#;
    let text = enml_to_text(&wrap_enml(html));
    assert_eq!(text, "This is **important**\n\n");
}

#[test]
fn converts_numeric_font_weight_to_markdown_bold() {
    let html = r#"<div>This is <span style="font-weight: 700;">important</span></div>"#;
    let text = enml_to_text(&wrap_enml(html));
    assert_eq!(text, "This is **important**\n\n");
}

#[test]
fn highlights_bold_for_terminal_output() {
    let text = enml_to_terminal_text(&wrap_enml("<div>This is <b>important</b></div>"));
    assert_eq!(text, "This is \x1b[1mimportant\x1b[0m\n\n");
}

#[test]
fn converts_links_to_markdown_links() {
    let text = enml_to_text(&wrap_enml(
        r#"<div>Open <a href="https://example.com">my clickable text</a></div>"#,
    ));
    assert_eq!(text, "Open [my clickable text](https://example.com)\n\n");
}

#[test]
fn unescapes_link_urls() {
    let text = enml_to_text(&wrap_enml(
        r#"<div><a href="https://example.com?a=1&amp;b=2">example</a></div>"#,
    ));
    assert_eq!(text, "[example](https://example.com?a=1&b=2)\n\n");
}

#[test]
fn keeps_link_text_when_href_is_missing() {
    let text = enml_to_text(&wrap_enml("<div><a>example</a></div>"));
    assert_eq!(text, "example\n\n");
}

#[test]
fn keeps_formatting_inside_markdown_links() {
    let text = enml_to_text(&wrap_enml(
        r#"<div><a href="https://example.com"><b>important</b></a></div>"#,
    ));
    assert_eq!(text, "[**important**](https://example.com)\n\n");
}

#[test]
fn highlights_links_for_terminal_output() {
    let text = enml_to_terminal_text(&wrap_enml(
        r#"<div>Open <a href="https://example.com">example</a></div>"#,
    ));
    assert_eq!(
        text,
        "Open \x1b[34m[example](https://example.com)\x1b[0m\n\n"
    );
}

#[test]
fn keeps_terminal_link_blue_after_nested_formatting_reset() {
    let text = enml_to_terminal_text(&wrap_enml(
        r#"<div><a href="https://example.com"><b>important</b></a></div>"#,
    ));
    assert_eq!(
        text,
        "\x1b[34m[\x1b[1mimportant\x1b[0m\x1b[34m](https://example.com)\x1b[0m\n\n"
    );
}

#[test]
fn keeps_greater_than_text() {
    let text = enml_to_text(&wrap_enml("<div>value > threshold</div>"));
    assert_eq!(text, "value > threshold\n\n");
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
    assert_eq!(text, "Run \x1b[38;5;81mcargo test\x1b[0m\n\n");
}

#[test]
fn converts_blockquotes_to_markdown_quotes() {
    let text = enml_to_text(&wrap_enml(
        "<blockquote><div>Quoted line</div><div>Second line</div></blockquote>",
    ));
    assert_eq!(text, "> Quoted line\n> Second line\n\n");
}

#[test]
fn converts_styled_quote_divs_to_markdown_quotes() {
    let html = r#"<div style="border-left: 3px solid rgb(200, 200, 200); padding-left: 12px;"><div>Styled quote</div></div>"#;
    let text = enml_to_text(&wrap_enml(html));
    assert_eq!(text, "> Styled quote\n\n");
}

#[test]
fn highlights_blockquotes_for_terminal_output() {
    let text = enml_to_terminal_text(&wrap_enml("<blockquote>Quoted line</blockquote>"));
    assert_eq!(
        text,
        "\x1b[38;5;39m|\x1b[0m \x1b[3;38;5;245mQuoted line\x1b[0m\n\n"
    );
}

fn resource(hash: &str, mime: &str, filename: &str) -> Resource {
    Resource {
        mime: Some(mime.to_string()),
        filename: filename.to_string(),
        data: ResourceData {
            body_hash: hash.to_string(),
            body: Vec::new(),
            size: 0,
        },
    }
}
