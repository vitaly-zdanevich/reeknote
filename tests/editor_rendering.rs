use reeknote::editor::{
    ImageInfo, ImageOptions, TextFormat, enml_to_terminal_text, enml_to_terminal_text_with_options,
    enml_to_text, enml_to_text_with_options, enml_to_text_with_resources, get_images, text_to_enml,
    wrap_enml,
};
use reeknote::models::{Resource, ResourceData};
use std::io::Cursor;

const MD_TEXT: &str = "# Header 1\n\n## Header 2\n\nLine 1\n\n_Line 2_\n\n**Line 3**\n\n";
const HTML_TEXT: &str = "<h1>Header 1</h1>\n<h2>Header 2</h2>\n<p>Line 1</p>\n<p><em>Line 2</em></p>\n<p><strong>Line 3</strong></p>\n";

fn expected_markdown_code_block(language: Option<&str>, body: &str) -> String {
    let language_style = language
        .map(|language| format!(" --en-syntaxLanguage:{language};"))
        .unwrap_or_default();
    format!(
        "<div style=\"--en-codeblock:true;{language_style} --en-lineWrapping:false;box-sizing: border-box; padding: 8px; font-family: &quot;Fira Code&quot;,&quot;Consolas&quot;,&quot;Monaco&quot;,&quot;Andale Mono&quot;,&quot;Ubuntu Mono&quot;,&quot;Courier New&quot;,monospace font-size: 12px; color: rgb(51, 51, 51); border-top-left-radius: 4px; border-top-right-radius: 4px; border-bottom-right-radius: 4px; border-bottom-left-radius: 4px; background-color: rgb(251, 250, 248); border: 1px solid rgba(0, 0, 0, 0.14902); background-position: initial initial; background-repeat: initial initial;\">{body}</div>"
    )
}

#[test]
fn converts_markdown_to_enml() {
    assert_eq!(text_to_enml(MD_TEXT), wrap_enml(HTML_TEXT));
}

#[test]
fn converts_markdown_fenced_code_blocks_to_enml() {
    let markdown = "Before\n\n```bash\nmy bash code\n```\n\nAfter";
    let html = format!(
        "<div>Before</div>{}<div>After</div><div><br/></div>",
        expected_markdown_code_block(Some("bash"), "<div>my bash code</div>")
    );
    assert_eq!(text_to_enml(markdown), wrap_enml(&html));
}

#[test]
fn converts_markdown_fenced_code_blocks_without_language_to_auto_code_blocks() {
    let markdown = "```\nif value < limit && value > 0\n```";
    let html = expected_markdown_code_block(
        None,
        "<div>if value &lt; limit &amp;&amp; value &gt; 0</div>",
    ) + "<div><br/></div>";
    assert_eq!(text_to_enml(markdown), wrap_enml(&html));
}

#[test]
fn converts_python_and_javascript_fence_languages_to_enml() {
    let markdown = "```python\nprint('hi')\n```\n\n```javascript\nconsole.log('hi')\n```";
    let html = format!(
        "{}{}<div><br/></div>",
        expected_markdown_code_block(Some("python"), "<div>print('hi')</div>"),
        expected_markdown_code_block(Some("javascript"), "<div>console.log('hi')</div>")
    );
    assert_eq!(text_to_enml(markdown), wrap_enml(&html));
}

#[test]
fn escapes_html_inside_markdown_fenced_code_blocks() {
    let markdown = "```\nif value < limit && value > 0\n```";
    let html = expected_markdown_code_block(
        None,
        "<div>if value &lt; limit &amp;&amp; value &gt; 0</div>",
    ) + "<div><br/></div>";
    assert_eq!(text_to_enml(markdown), wrap_enml(&html));
}

#[test]
fn keeps_angle_bracket_placeholders_inside_markdown_code_blocks() {
    let note = text_to_enml("```\ngit tag <name>\n```");
    assert_eq!(enml_to_text(&note), "```\ngit tag <name>\n```\n\n");
}

#[test]
fn keeps_angle_bracket_placeholders_inside_inline_code() {
    let text = enml_to_text(&wrap_enml(
        "<div>Run <code>git tag &lt;name&gt;</code></div>",
    ));
    assert_eq!(text, "Run `git tag <name>`\n\n");
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
fn renders_images_for_kitty_terminal_output() {
    let note = wrap_enml(r#"<en-media type="image/png" hash="abc" />"#);
    let mut resource = resource("abc", "image/png", "photo.png");
    resource.data.body = png_1x1();
    resource.data.size = resource.data.body.len();
    let text = enml_to_terminal_text_with_options(&note, &[resource], true);
    assert!(text.contains("\x1b_Ga=T,t=f,f=100,q=2;"));
    assert!(text.contains("\x1b\\photo.png\n\n"));
    assert!(!text.contains("[Image:"));
}

#[test]
fn renders_multiple_images_with_filenames_for_kitty_terminal_output() {
    let note = wrap_enml(
        r#"<en-media type="image/png" hash="abc" /><en-media type="image/png" hash="def" />"#,
    );
    let mut first = resource("abc", "image/png", "first.png");
    first.data.body = png_1x1();
    first.data.size = first.data.body.len();
    let mut second = resource("def", "image/png", "second.png");
    second.data.body = png_1x1();
    second.data.size = second.data.body.len();

    let text = enml_to_terminal_text_with_options(&note, &[first, second], true);

    assert_eq!(text.matches("\x1b_Ga=T,t=f,f=100,q=2;").count(), 2);
    assert!(text.contains("\x1b\\first.png\n\n"));
    assert!(text.contains("\x1b\\second.png\n\n"));
}

#[test]
fn falls_back_to_image_placeholder_when_terminal_image_data_is_missing() {
    let note = wrap_enml(r#"<en-media type="image/png" hash="abc" />"#);
    let text = enml_to_terminal_text_with_options(
        &note,
        &[resource("abc", "image/png", "photo.png")],
        true,
    );
    assert_eq!(text, "[Image: photo.png]\n\n");
}

#[test]
fn falls_back_to_image_placeholder_when_terminal_image_data_is_invalid() {
    let note = wrap_enml(r#"<en-media type="image/png" hash="abc" />"#);
    let mut resource = resource("abc", "image/png", "photo.png");
    resource.data.body = b"not an image".to_vec();
    resource.data.size = resource.data.body.len();
    let text = enml_to_terminal_text_with_options(&note, &[resource], true);
    assert_eq!(text, "[Image: photo.png]\n\n");
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
fn keeps_markdown_link_syntax_when_label_matches_href() {
    let text = enml_to_text(&wrap_enml(
        r#"<div>Open <a href="https://example.com">https://example.com</a></div>"#,
    ));
    assert_eq!(text, "Open [https://example.com](https://example.com)\n\n");
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
fn shows_terminal_link_without_markdown_syntax_when_label_matches_href() {
    let text = enml_to_terminal_text(&wrap_enml(
        r#"<div>Open <a href="https://example.com">https://example.com</a></div>"#,
    ));
    assert_eq!(text, "Open \x1b[34mhttps://example.com\x1b[0m\n\n");
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
        guid: String::new(),
        mime: Some(mime.to_string()),
        filename: filename.to_string(),
        data: ResourceData {
            body_hash: hash.to_string(),
            body: Vec::new(),
            size: 0,
        },
    }
}

fn png_1x1() -> Vec<u8> {
    let image = image::RgbaImage::from_pixel(1, 1, image::Rgba([0, 0, 0, 255]));
    let mut bytes = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image)
        .write_to(&mut bytes, image::ImageFormat::Png)
        .expect("test PNG should encode");
    bytes.into_inner()
}
