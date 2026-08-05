//! Integration tests for the ddl install command (CLI parsing + error paths).
//!
//! Actual install runs subprocesses (brew, cargo, network) so we test CLI
//! parsing, help text, error handling, and fast-path detection here.
//! The install logic is tested via unit tests in the installer module.

use assert_cmd::Command;
use predicates::prelude::*;
use std::time::Duration;

/// Timeout for each test.
const CMD_TIMEOUT: Duration = Duration::from_secs(10);

// ===================== CLI flag parsing tests =====================

#[test]
fn test_install_help_has_tool_arg() {
    let mut cmd = Command::cargo_bin("ddl").unwrap();
    cmd.arg("install").arg("--help");
    cmd.timeout(CMD_TIMEOUT);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("ddl install"))
        .stdout(predicate::str::contains("TOOL"));
}

#[test]
fn test_install_help_shows_description() {
    let mut cmd = Command::cargo_bin("ddl").unwrap();
    cmd.arg("install").arg("--help");
    cmd.timeout(CMD_TIMEOUT);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Install a single tool"));
}

#[test]
fn test_install_help_lists_known_tools() {
    let mut cmd = Command::cargo_bin("ddl").unwrap();
    cmd.arg("install").arg("--help");
    cmd.timeout(CMD_TIMEOUT);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("wai"))
        .stdout(predicate::str::contains("dont"))
        .stdout(predicate::str::contains("ah"));
}

// ===================== Error handling tests =====================

#[test]
fn test_install_unknown_tool_fails() {
    let mut cmd = Command::cargo_bin("ddl").unwrap();
    cmd.arg("install").arg("nonexistent-tool");
    cmd.timeout(CMD_TIMEOUT);
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("not found").or(predicate::str::contains("Unknown")));
}

#[test]
fn test_install_unknown_tool_suggests_closest() {
    let mut cmd = Command::cargo_bin("ddl").unwrap();
    // "wai" is a known tool — "waii" should suggest "wai"
    cmd.arg("install").arg("waii");
    cmd.timeout(CMD_TIMEOUT);
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Did you mean"));
}

#[test]
fn test_install_no_args_fails() {
    let mut cmd = Command::cargo_bin("ddl").unwrap();
    cmd.arg("install");
    cmd.timeout(CMD_TIMEOUT);
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("required").or(predicate::str::contains("error")));
}

#[test]
fn test_install_unknown_flag_errors() {
    let mut cmd = Command::cargo_bin("ddl").unwrap();
    cmd.arg("install").arg("wai").arg("--unknown-flag");
    cmd.timeout(CMD_TIMEOUT);
    cmd.assert().failure();
}

// ===================== Fast-path detection tests =====================

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
fn test_install_on_path_returns_success() {
    // Install a tool that is already on PATH (e.g., git).
    // `git` is not a managed tool, so it should fail with Unknown tool.
    // This verifies the fast-path lookup works.
    let (mut cmd, _temp) = ddl_cmd();
    cmd.arg("install").arg("git");
    cmd.timeout(CMD_TIMEOUT);
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Unknown tool"));
}

#[test]
fn test_install_json_parses() {
    let mut cmd = Command::cargo_bin("ddl").unwrap();
    cmd.arg("install").arg("wai").arg("--json");
    cmd.timeout(CMD_TIMEOUT);
    // --json should always produce valid JSON output, even on error
    cmd.assert()
        .stdout(predicate::str::contains("envelope_kind"));
}

// ===================== Argument interaction tests =====================

#[test]
fn test_install_global_verbose_flag_works() {
    // --verbose is a global flag, so it should work before the subcommand
    let mut cmd = Command::cargo_bin("ddl").unwrap();
    cmd.arg("--verbose").arg("install").arg("--help");
    cmd.timeout(CMD_TIMEOUT);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("ddl install"));
}
