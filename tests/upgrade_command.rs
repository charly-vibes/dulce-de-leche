//! Integration tests for the ddl upgrade command (CLI parsing only).
//!
//! Upgrade actually runs subprocesses (brew, cargo, network) so we only
//! test CLI parsing and help text here. The upgrade logic is tested via
//! unit tests in the installer module.

use assert_cmd::Command;
use predicates::prelude::*;
use std::time::Duration;

/// Timeout for each test.
const CMD_TIMEOUT: Duration = Duration::from_secs(10);

// ===================== CLI flag parsing tests =====================

#[test]
fn test_upgrade_help_has_tool_arg() {
    let mut cmd = Command::cargo_bin("ddl").unwrap();
    cmd.arg("upgrade").arg("--help");
    cmd.timeout(CMD_TIMEOUT);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("ddl upgrade"))
        .stdout(predicate::str::contains("TOOL"));
}

#[test]
fn test_upgrade_help_shows_description() {
    let mut cmd = Command::cargo_bin("ddl").unwrap();
    cmd.arg("upgrade").arg("--help");
    cmd.timeout(CMD_TIMEOUT);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Update all tools"));
}

#[test]
fn test_upgrade_unknown_tool_is_fast() {
    let (mut cmd, _temp) = ddl_cmd();
    cmd.arg("upgrade").arg("nonexistent-tool");
    cmd.timeout(CMD_TIMEOUT);
    // Unknown tool names are validated before any network/subprocess calls
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("not found").or(predicate::str::contains("error")));
}

#[test]
fn test_upgrade_unknown_flag_errors() {
    let mut cmd = Command::cargo_bin("ddl").unwrap();
    cmd.arg("upgrade").arg("--unknown-flag");
    cmd.timeout(CMD_TIMEOUT);
    cmd.assert().failure();
}

/// Helper: create a temp directory and return the cmd.
fn ddl_cmd() -> (Command, tempfile::TempDir) {
    let temp = tempfile::tempdir().unwrap();
    let mut cmd = Command::cargo_bin("ddl").unwrap();
    cmd.current_dir(temp.path());
    (cmd, temp)
}