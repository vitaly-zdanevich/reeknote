use reeknote::editor::edit_content;

#[test]
fn edits_content_with_external_command() {
    let outcome = edit_content("sed -i s/original/edited/", "original", ".md").unwrap();
    assert_eq!(outcome.content, "edited");
    assert!(outcome.changed);
    assert!(!outcome.path.exists());
}
