//! Integration tests for the ddl status command.
//!
//! The status command shows the ecosystem health overview.
//! Note: status scans the system for installed tools, so even without a
//! .ddl/ manifest it will report tools found on PATH.

use assert_cmd::Command;
use predicates::prelude::*;
use std::time::Duration;

/// Timeout for each test.
const CMD_TIMEOUT: Duration = Duration::from_secs(10);

/// Helper: create a temp dir with a `.ddl/` manifest.
fn ddl_cmd() -> (Command, tempfile::TempDir) {
    let temp = tempfile::tempdir().unwrap();
    let manifest_dir = temp.path().join(".ddl");
    std::fs::create_dir_all(&manifest_dir).unwrap();
    let manifest = serde_json::json!({
        "ddl_version": "0.3.0",
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
fn test_status_help_shows_description() {
    let mut cmd = Command::cargo_bin("ddl").unwrap();
    cmd.arg("status").arg("--help");
    cmd.timeout(CMD_TIMEOUT);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("ddl status"));
}

#[test]
fn test_status_without_ddl_dir_shows_ecosystem_header() {
    // Even without .ddl/, status detects tools on PATH.
    let temp = tempfile::tempdir().unwrap();
    let mut cmd = Command::cargo_bin("ddl").unwrap();
    cmd.current_dir(temp.path());
    cmd.arg("status").arg("--human");
    cmd.timeout(CMD_TIMEOUT);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("ecosystem status"));
}

#[test]
fn test_status_with_ddl_dir_shows_ecosystem_header() {
    let (mut cmd, _temp) = ddl_cmd();
    cmd.arg("status").arg("--human");
    cmd.timeout(CMD_TIMEOUT);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("ecosystem status"));
}

#[test]
fn test_status_json_output() {
    let (mut cmd, _temp) = ddl_cmd();
    cmd.arg("status").arg("--json");
    cmd.timeout(CMD_TIMEOUT);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("envelope_kind"))
        .stdout(predicate::str::contains("sections"));
}

#[test]
fn test_status_unknown_flag_errors() {
    let mut cmd = Command::cargo_bin("ddl").unwrap();
    cmd.arg("status").arg("--unknown-flag");
    cmd.timeout(CMD_TIMEOUT);
    cmd.assert().failure();
}
