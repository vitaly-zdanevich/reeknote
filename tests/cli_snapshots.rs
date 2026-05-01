use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[test]
fn reeknote_about_snapshot() {
    assert_cli_snapshot("reeknote_about", reeknote_bin(), &[], None);
}

#[test]
fn reeknote_global_help_snapshot() {
    assert_cli_snapshot("reeknote_global_help", reeknote_bin(), &["--help"], None);
}

#[test]
fn reeknote_find_help_snapshot() {
    assert_cli_snapshot(
        "reeknote_find_help",
        reeknote_bin(),
        &["find", "--help"],
        None,
    );
}

#[test]
fn reeknote_settings_snapshot() {
    let app_dir = isolated_app_dir("settings");
    let _ = fs::remove_dir_all(&app_dir);
    assert_cli_snapshot(
        "reeknote_settings",
        reeknote_bin(),
        &["settings"],
        Some(&app_dir),
    );
    let _ = fs::remove_dir_all(app_dir);
}

#[test]
fn reeknote_invalid_command_snapshot() {
    assert_cli_snapshot("reeknote_invalid_command", reeknote_bin(), &["nope"], None);
}

#[test]
fn gnsync_help_snapshot() {
    assert_cli_snapshot("gnsync_help", gnsync_bin(), &["--help"], None);
}

fn assert_cli_snapshot(name: &str, bin: &str, args: &[&str], app_dir: Option<&Path>) {
    let output = run_cli(bin, args, app_dir);
    let snapshot_path = snapshot_path(name);

    if std::env::var_os("REEKNOTE_UPDATE_SNAPSHOTS").is_some() {
        fs::write(&snapshot_path, &output).expect("write snapshot");
        return;
    }

    let expected = fs::read_to_string(&snapshot_path).expect("read snapshot");
    assert_eq!(
        expected, output,
        "CLI snapshot mismatch for {name}. To accept the new output, run with REEKNOTE_UPDATE_SNAPSHOTS=1."
    );
}

fn run_cli(bin: &str, args: &[&str], app_dir: Option<&Path>) -> String {
    let mut command = Command::new(bin);
    command
        .args(args)
        .env_remove("EDITOR")
        .env_remove("editor")
        .env_remove("EVERNOTE_DEV_TOKEN");

    if let Some(app_dir) = app_dir {
        command.env("REEKNOTE_APP_DIR", app_dir);
    }

    let output = command.output().expect("run CLI command");
    let mut stdout = String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n");
    let mut stderr = String::from_utf8_lossy(&output.stderr).replace("\r\n", "\n");

    if let Some(app_dir) = app_dir {
        let app_dir = app_dir.display().to_string();
        stdout = stdout.replace(&app_dir, "<APP_DIR>");
        stderr = stderr.replace(&app_dir, "<APP_DIR>");
    }

    snapshot_output(
        output.status.code().unwrap_or(-1),
        &normalize_section(&stdout),
        &normalize_section(&stderr),
    )
}

fn snapshot_output(status: i32, stdout: &str, stderr: &str) -> String {
    format!("status: {status}\n--- stdout ---\n{stdout}--- stderr ---\n{stderr}")
}

fn normalize_section(output: &str) -> String {
    let output = output.replace('\\', "/");
    let output = output.trim_end_matches('\n');
    if output.is_empty() {
        String::new()
    } else {
        format!("{output}\n")
    }
}

fn snapshot_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("snapshots")
        .join(format!("{name}.snap"))
}

fn isolated_app_dir(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "reeknote-cli-snapshot-{name}-{}",
        std::process::id()
    ))
}

fn reeknote_bin() -> &'static str {
    env!("CARGO_BIN_EXE_reeknote")
}

fn gnsync_bin() -> &'static str {
    env!("CARGO_BIN_EXE_gnsync")
}
