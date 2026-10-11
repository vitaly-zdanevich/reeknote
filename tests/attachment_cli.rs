//! Exercise download command dispatch and early failures using isolated CLI processes.

use std::fs;
use std::process::Command;

/// Missing selectors must fail before opening storage or creating local files.
#[test]
fn requires_note_before_accessing_storage() {
    let directory = tempfile::tempdir().unwrap();
    let app_dir = directory.path().join("config");
    let output = Command::new(env!("CARGO_BIN_EXE_reeknote"))
        .arg("download")
        .current_dir(directory.path())
        .env("REEKNOTE_APP_DIR", &app_dir)
        .env_remove("EVERNOTE_DEV_TOKEN")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("--note"));
    assert!(!app_dir.exists());
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 0);
}

/// Download reaches authentication, without modifying the note or local files.
#[test]
fn requires_login_without_creating_attachments() {
    let directory = tempfile::tempdir().unwrap();
    let app_dir = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_reeknote"))
        .args(["download", "00000000-0000-0000-0000-000000000001"])
        .current_dir(directory.path())
        .env("REEKNOTE_APP_DIR", app_dir.path())
        .env_remove("EVERNOTE_DEV_TOKEN")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("not logged in"));
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 0);
}
