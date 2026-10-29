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

/// Reproduce issue #4 using the attributed heading from the updated test note.
#[test]
fn highlights_evernote_heading_on_its_own_line() {
    let note = concat!(
        "<!DOCTYPE en-note SYSTEM \"http://xml.evernote.com/pub/enml2.dtd\">",
        "<en-note><h1 style=\"--en-nodeId:884eaf5b-6fa8-43b8-ad59-0125c7c289f0;\">",
        "My header</h1><div>My second line</div></en-note>",
    );
    assert_eq!(
        enml_to_terminal_text(note),
        "\x1b[1;33mMy header\x1b[0m\n\nMy second line\n\n",
    );
    assert_eq!(enml_to_text(note), "# My header\n\nMy second line\n\n");
}

/// All supported heading levels retain their Markdown meaning and terminal style.
#[test]
fn renders_headings_with_and_without_attributes() {
    for level in 1..=3 {
        for attributes in ["", " style=\"--en-nodeId:heading;\"", " title='Heading'"] {
            let note = wrap_enml(&format!(
                "<div>Before</div><h{level}{attributes}><span>Git &lt;name&gt;</span></h{level}><div>After</div>",
            ));
            assert_eq!(
                enml_to_text(&note),
                format!("Before\n\n{} Git <name>\n\nAfter\n\n", "#".repeat(level)),
            );
            assert_eq!(
                enml_to_terminal_text(&note),
                "Before\n\n\x1b[1;33mGit <name>\x1b[0m\n\nAfter\n\n",
            );
        }
    }
}

/// Inline style resets must restore heading styling without coloring following text.
#[test]
fn restores_heading_color_after_inline_formatting() {
    let note = wrap_enml("<h1><i>Important</i> heading</h1><div>After</div>");
    assert_eq!(
        enml_to_terminal_text(&note),
        "\x1b[1;33m\x1b[3mImportant\x1b[0m\x1b[1;33m heading\x1b[0m\n\nAfter\n\n",
    );
    assert_eq!(enml_to_text(&note), "# _Important_ heading\n\nAfter\n\n");
}

/// Heading spacing must not remove the explicit blank lines fixed in issue #5.
#[test]
fn preserves_explicit_blank_lines_around_headings() {
    let note = wrap_enml(concat!(
        "<div>Before</div><div><br/></div><div><br/></div><div><br/></div>",
        "<h1 style=\"--en-nodeId:heading;\">Heading</h1>",
        "<div><br/></div><div><br/></div><div>After</div>",
    ));
    assert_eq!(
        enml_to_terminal_text(&note),
        "Before\n\n\n\n\x1b[1;33mHeading\x1b[0m\n\n\nAfter\n\n",
    );
    assert_eq!(
        enml_to_text(&note),
        "Before\n\n\n\n# Heading\n\n\nAfter\n\n"
    );
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

/// Adjacent Evernote divs are consecutive lines, not separate paragraphs.
#[test]
fn converts_div_blocks_to_lines() {
    let html = "<div>First line</div><div>Second line</div>";
    assert_eq!(
        enml_to_text(&wrap_enml(html)),
        "First line\nSecond line\n\n"
    );
}

/// Reproduce the spacing in the user's read-only "My note formatting test" note.
#[test]
fn preserves_consecutive_lines_and_the_explicit_gap_in_the_reported_note() {
    let note = concat!(
        "<!DOCTYPE en-note SYSTEM \"http://xml.evernote.com/pub/enml2.dtd\">",
        "<en-note><div>This is my first line</div><div>This is my second line</div>",
        "<div><br/></div>",
        "<div>This is my third line - after the empty line delimeter.</div></en-note>",
    );
    let expected = concat!(
        "This is my first line\nThis is my second line\n\n",
        "This is my third line - after the empty line delimeter.\n\n",
    );
    for output in [
        enml_to_text(note),
        enml_to_terminal_text(note),
        enml_to_text_with_options(note, TextFormat::Plain, &ImageOptions::default()),
    ] {
        assert_eq!(output, expected);
    }
}

/// Node attributes and container divs must not change line spacing.
#[test]
fn preserves_consecutive_lines_in_attributed_and_nested_divs() {
    for attributes in ["", " style=\"--en-nodeId:test;\""] {
        let lines = format!(
            "<div{attributes}>First</div>\n<div{attributes}>Second</div>\n\
             <div{attributes}><br/></div>\n<div{attributes}>Third</div>",
        );
        for body in [lines.clone(), format!("<div{attributes}>{lines}</div>")] {
            let note = wrap_enml(&body);
            assert_eq!(enml_to_text(&note), "First\nSecond\n\nThird\n\n");
            assert_eq!(enml_to_terminal_text(&note), "First\nSecond\n\nThird\n\n");
        }
    }
}

/// Compact line runs must retain spacing around actual paragraphs and code blocks.
#[test]
fn keeps_paragraph_and_code_spacing_after_consecutive_lines() {
    let note = wrap_enml(concat!(
        "<div>First</div><div>Second</div><p>A paragraph</p>",
        "<pre>let value = 1;</pre><div>Last</div>",
    ));
    assert_eq!(
        enml_to_text(&note),
        "First\nSecond\n\nA paragraph\n\n```\nlet value = 1;\n```\n\nLast\n\n",
    );
}

/// Line compaction must retain the padding added by image, code, and quote renderers.
#[test]
fn keeps_rendered_blocks_separate_from_following_div_lines() {
    for block in [
        "<en-media type=\"image/png\" hash=\"abc\"/>",
        "<pre>let value = 1;</pre>",
        "<blockquote>A quote</blockquote>",
    ] {
        let note = wrap_enml(&format!(
            "<div>{block}</div><div>After</div><div>Then</div>",
        ));
        for output in [enml_to_text(&note), enml_to_terminal_text(&note)] {
            assert!(
                output.ends_with("\n\nAfter\nThen\n\n"),
                "Output: {output:?}"
            );
        }
    }
}

/// Reproduce issue #5 with the ENML fetched from the reported test note.
#[test]
fn preserves_evernote_blank_lines_in_terminal_output() {
    let note = concat!(
        "<!DOCTYPE en-note SYSTEM \"http://xml.evernote.com/pub/enml2.dtd\">",
        "<en-note><div>My first line</div>",
        "<div><br/></div><div><br/></div><div><br/></div>",
        "<div>My second line</div></en-note>",
    );
    assert_eq!(
        enml_to_terminal_text(note),
        "My first line\n\n\n\nMy second line\n\n",
    );
}

/// Keep explicit blank lines in editable Markdown and plain-text exports too.
#[test]
fn preserves_evernote_blank_lines_in_text_output() {
    let note = wrap_enml(concat!(
        "<div>My first line</div>",
        "<div><br/></div><div><br/></div><div><br/></div>",
        "<div>My second line</div>",
    ));
    let expected = "My first line\n\n\n\nMy second line\n\n";
    assert_eq!(enml_to_text(&note), expected);
    assert_eq!(
        enml_to_text_with_options(&note, TextFormat::Plain, &ImageOptions::default()),
        expected,
    );
}

/// Source line endings must not add to the number of explicitly empty blocks.
#[test]
fn preserves_evernote_blank_lines_without_adding_paragraph_spacing() {
    for line_break in ["<br/>", "<br />", "<br>"] {
        for separator in ["", "\n", "\r\n"] {
            for blank_lines in 0..=3 {
                let gap = format!("<div>{line_break}</div>{separator}").repeat(blank_lines);
                let note = wrap_enml(&format!(
                    "<div>Before</div>{separator}{gap}<div>After</div>",
                ));
                let expected = format!("Before{}After\n\n", "\n".repeat(blank_lines + 1));
                assert_eq!(enml_to_text(&note), expected, "ENML: {note}");
                assert_eq!(enml_to_terminal_text(&note), expected, "ENML: {note}");
            }
        }
    }
}

/// Explicit empty blocks remain significant even without text on both sides.
#[test]
fn preserves_evernote_blank_lines_at_note_boundaries() {
    let blank_lines = "<div><br/></div>".repeat(3);
    for (html, expected) in [
        (format!("{blank_lines}<div>After</div>"), "\n\n\nAfter\n\n"),
        (format!("<div>Before</div>{blank_lines}"), "Before\n\n\n\n"),
        (blank_lines, "\n\n\n"),
    ] {
        let note = wrap_enml(&html);
        assert_eq!(enml_to_text(&note), expected, "ENML: {note}");
        assert_eq!(enml_to_terminal_text(&note), expected, "ENML: {note}");
    }
}

#[test]
fn renders_horizontal_rules_as_text_separators() {
    let html = "<div>Before</div><hr/><div>After</div>";
    assert_eq!(
        enml_to_text(&wrap_enml(html)),
        "Before\n\n------------------------\n\nAfter\n\n"
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
    assert!(text.contains("\x1b\\\n\x1b[3mphoto.png\x1b[0m\n\n"));
    assert!(!text.contains("[Image:"));
}

#[test]
fn renders_webp_images_even_when_evernote_labels_them_png() {
    let note = wrap_enml(r#"<en-media type="image/png" hash="abc" />"#);
    let mut resource = resource("abc", "image/png", "photo.png");
    resource.data.body = webp_1x1();
    resource.data.size = resource.data.body.len();
    let text = enml_to_terminal_text_with_options(&note, &[resource], true);
    assert!(text.contains("\x1b_Ga=T,t=f,f=100,q=2;"));
    assert!(text.contains("\x1b\\\n\x1b[3mphoto.png\x1b[0m\n\n"));
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
    assert!(text.contains("\x1b\\\n\x1b[3mfirst.png\x1b[0m\n\n"));
    assert!(text.contains("\x1b\\\n\x1b[3msecond.png\x1b[0m\n\n"));
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
fn prepends_emoji_to_audio_attachment_placeholders() {
    let note = wrap_enml(r#"<en-media type="audio/mpeg" hash="abc" />"#);
    let resources = vec![resource("abc", "audio/mpeg", "voice.mp3")];
    assert_eq!(
        enml_to_text_with_resources(&note, &resources),
        "🎧 [Attachment: voice.mp3]\n\n"
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

fn webp_1x1() -> Vec<u8> {
    let image = image::RgbaImage::from_pixel(1, 1, image::Rgba([0, 0, 0, 255]));
    let mut bytes = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image)
        .write_to(&mut bytes, image::ImageFormat::WebP)
        .expect("test WebP should encode");
    bytes.into_inner()
}
