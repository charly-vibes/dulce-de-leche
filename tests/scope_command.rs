//! Integration tests for the ddl scope command.
//!
//! The scope command shows the active .ddl/ directory by walking up from CWD.

use assert_cmd::Command;
use predicates::prelude::*;
use std::time::Duration;

/// Timeout for each test.
const CMD_TIMEOUT: Duration = Duration::from_secs(10);

/// Helper: create a temp dir with a `.ddl/` directory.
fn ddl_cmd() -> (Command, tempfile::TempDir) {
    let temp = tempfile::tempdir().unwrap();
    let manifest_dir = temp.path().join(".ddl");
    std::fs::create_dir_all(&manifest_dir).unwrap();
    let manifest = serde_json::json!({
        "ddl_version": "0.2.0",
        "migration_state": "none",
        "tools": {}
    });
    std::fs::write(
        manifest_dir.join("manifest.json"),
        serde_json::to_string_pretty(&manifest).unwrap(),
    )
    .unwrap();
    let mut cmd = Command::cargo_bin("ddl").unwrap();
    cmd.current_dir(temp.path());
    cmd.timeout(CMD_TIMEOUT);
    (cmd, temp)
}

#[test]
fn test_scope_help_shows_description() {
    let mut cmd = Command::cargo_bin("ddl").unwrap();
    cmd.arg("scope").arg("--help");
    cmd.timeout(CMD_TIMEOUT);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("ddl scope"));
}

#[test]
fn test_scope_with_ddl_dir_shows_path() {
    let (mut cmd, temp) = ddl_cmd();
    cmd.arg("scope").arg("--human");
    cmd.timeout(CMD_TIMEOUT);
    cmd.assert().success().stdout(predicate::str::contains(
        temp.path().to_string_lossy().as_ref(),
    ));
}

#[test]
fn test_scope_without_ddl_dir_shows_message() {
    let temp = tempfile::tempdir().unwrap();
    let mut cmd = Command::cargo_bin("ddl").unwrap();
    cmd.current_dir(temp.path());
    cmd.arg("scope").arg("--human");
    cmd.timeout(CMD_TIMEOUT);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("No .ddl/ directory found"));
}

#[test]
fn test_scope_json_with_ddl_dir() {
    let (mut cmd, _temp) = ddl_cmd();
    cmd.arg("scope").arg("--json");
    cmd.timeout(CMD_TIMEOUT);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("envelope_kind"))
        .stdout(predicate::str::contains("ddl_dir"));
}

#[test]
fn test_scope_json_without_ddl_dir() {
    let temp = tempfile::tempdir().unwrap();
    let mut cmd = Command::cargo_bin("ddl").unwrap();
    cmd.current_dir(temp.path());
    cmd.arg("scope").arg("--json");
    cmd.timeout(CMD_TIMEOUT);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("envelope_kind"))
        .stdout(predicate::str::contains("ddl_dir"));
}

#[test]
fn test_scope_unknown_flag_errors() {
    let mut cmd = Command::cargo_bin("ddl").unwrap();
    cmd.arg("scope").arg("--unknown-flag");
    cmd.timeout(CMD_TIMEOUT);
    cmd.assert().failure();
}
