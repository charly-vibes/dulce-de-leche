//! Integration tests for the ddl version command.
//!
//! The version command prints the ddl version and lists installed tools.
//! It must NOT create a .ddl/ directory when run outside one.

use assert_cmd::Command;
use predicates::prelude::*;
use std::time::Duration;

/// Timeout for each test.
const CMD_TIMEOUT: Duration = Duration::from_secs(10);

#[test]
fn test_version_help_shows_description() {
    let mut cmd = Command::cargo_bin("ddl").unwrap();
    cmd.arg("version").arg("--help");
    cmd.timeout(CMD_TIMEOUT);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("ddl version"));
}

#[test]
fn test_version_prints_ddl_version() {
    let mut cmd = Command::cargo_bin("ddl").unwrap();
    cmd.arg("version").arg("--human");
    cmd.timeout(CMD_TIMEOUT);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("ddl:"));
}

#[test]
fn test_version_does_not_create_ddl_dir() {
    let temp = tempfile::tempdir().unwrap();
    let mut cmd = Command::cargo_bin("ddl").unwrap();
    cmd.current_dir(temp.path());
    cmd.arg("version").arg("--human");
    cmd.timeout(CMD_TIMEOUT);
    cmd.assert().success();
    assert!(
        !temp.path().join(".ddl").exists(),
        "version must not create .ddl/ directory"
    );
}

#[test]
fn test_version_json_output() {
    let mut cmd = Command::cargo_bin("ddl").unwrap();
    cmd.arg("version").arg("--json");
    cmd.timeout(CMD_TIMEOUT);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("envelope_kind"))
        .stdout(predicate::str::contains("version"));
}

#[test]
fn test_version_unknown_flag_errors() {
    let mut cmd = Command::cargo_bin("ddl").unwrap();
    cmd.arg("version").arg("--unknown-flag");
    cmd.timeout(CMD_TIMEOUT);
    cmd.assert().failure();
}

#[test]
fn test_version_json_does_not_create_ddl_dir() {
    let temp = tempfile::tempdir().unwrap();
    let mut cmd = Command::cargo_bin("ddl").unwrap();
    cmd.current_dir(temp.path());
    cmd.arg("version").arg("--json");
    cmd.timeout(CMD_TIMEOUT);
    cmd.assert().success();
    assert!(
        !temp.path().join(".ddl").exists(),
        "--json version must not create .ddl/ directory"
    );
}

#[test]
fn test_version_global_verbose_flag_works() {
    let mut cmd = Command::cargo_bin("ddl").unwrap();
    cmd.arg("--verbose").arg("version").arg("--help");
    cmd.timeout(CMD_TIMEOUT);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("ddl version"));
}
