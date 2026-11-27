//! Integration tests for `ddl status` edge cases (health spec):
//! discovery mode without a manifest, missing-tool marking, and exit codes.

mod common;

use assert_cmd::Command;
use predicates::prelude::*;
use std::time::Duration;

/// Timeout for each test.
const CMD_TIMEOUT: Duration = Duration::from_secs(120);

fn ddl_cmd(temp: &tempfile::TempDir) -> Command {
    let mut cmd = Command::cargo_bin("ddl").unwrap();
    common::clean_git_env(&mut cmd);
    cmd.current_dir(temp.path());
    cmd.timeout(CMD_TIMEOUT);
    cmd
}

#[test]
fn test_status_without_manifest_suggests_init() {
    // No `.ddl/` at all: status falls back to PATH discovery and points at
    // `ddl init` (health/discovery-mode-no-manifest).
    let temp = tempfile::tempdir().unwrap();
    let output = ddl_cmd(&temp)
        .args(["--human", "status"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(output).unwrap();
    assert!(
        text.to_lowercase().contains("init"),
        "status without manifest must suggest `ddl init`, got:\n{text}"
    );
}

#[test]
fn test_status_with_ddl_dir_but_no_manifest_reports_missing() {
    // `.ddl/` exists but manifest.json is missing: status still reports
    // and points at init (dot-ddl/manifest-missing).
    let temp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(temp.path().join(".ddl")).unwrap();
    let output = ddl_cmd(&temp)
        .args(["--human", "status"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(output).unwrap();
    assert!(
        text.to_lowercase().contains("init"),
        "status with empty .ddl/ must suggest `ddl init`, got:\n{text}"
    );
}

#[test]
fn test_status_marks_missing_tools_with_install_suggestion() {
    // Managed tools that are not on PATH are marked "not installed" with
    // their census category (health/some-tools-not-installed). The sandbox
    // has most tools installed; anything missing renders the marker.
    let temp = tempfile::tempdir().unwrap();
    let output = ddl_cmd(&temp)
        .args(["--human", "status"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(output).unwrap();
    assert!(
        text.contains("not installed ["),
        "status must mark uninstalled tools with 'not installed [<category>]', got:\n{text}"
    );
}

#[test]
fn test_status_json_carries_tool_fields() {
    // health/status-as-json: JSON envelope data carries per-tool entries.
    let temp = tempfile::tempdir().unwrap();
    ddl_cmd(&temp)
        .args(["status", "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("envelope_kind"))
        .stdout(predicate::str::contains("sections"));
}
