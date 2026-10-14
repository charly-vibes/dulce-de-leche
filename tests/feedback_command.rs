//! Integration tests for the ddl feedback command (DDL-isi).
//!
//! Wires `ddl feedback <kind>` through `genesis::feedback::handle_feedback`,
//! with error-scratch recording so `--from-last-error` works.

mod common;
use assert_cmd::Command;
use predicates::prelude::*;
use std::time::Duration;

const CMD_TIMEOUT: Duration = Duration::from_secs(10);

#[test]
fn test_feedback_help_shows_kinds() {
    let mut cmd = Command::cargo_bin("ddl").unwrap();
    common::clean_git_env(&mut cmd);
    cmd.arg("feedback").arg("--help");
    cmd.timeout(CMD_TIMEOUT);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("bug"))
        .stdout(predicate::str::contains("feature"));
}

#[test]
fn test_feedback_invalid_kind_fails() {
    let temp = tempfile::tempdir().unwrap();
    let mut cmd = Command::cargo_bin("ddl").unwrap();
    common::clean_git_env(&mut cmd);
    cmd.current_dir(temp.path());
    cmd.arg("feedback")
        .arg("bugz")
        .arg("--dry-run")
        .write_stdin("");
    cmd.timeout(CMD_TIMEOUT);
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("unknown kind"));
}

#[test]
fn test_feedback_no_content_fails() {
    let temp = tempfile::tempdir().unwrap();
    let mut cmd = Command::cargo_bin("ddl").unwrap();
    common::clean_git_env(&mut cmd);
    cmd.current_dir(temp.path());
    cmd.arg("feedback")
        .arg("bug")
        .arg("--dry-run")
        .write_stdin("");
    cmd.timeout(CMD_TIMEOUT);
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("No issue content specified"));
}

#[test]
fn test_feedback_dry_run_from_stdin() {
    let temp = tempfile::tempdir().unwrap();
    let mut cmd = Command::cargo_bin("ddl").unwrap();
    common::clean_git_env(&mut cmd);
    cmd.current_dir(temp.path());
    cmd.arg("feedback")
        .arg("bug")
        .arg("--dry-run")
        .write_stdin("Crash on init\nwith stack trace\n");
    cmd.timeout(CMD_TIMEOUT);
    cmd.assert()
        .success()
        .stderr(predicate::str::contains("[bug] Crash on init"))
        .stderr(predicate::str::contains("## Description"))
        .stderr(predicate::str::contains(
            "Would file: gh issue create --repo charly-vibes/dulce-de-leche",
        ));
}

#[test]
fn test_feedback_dry_run_multi_line_promotes_title() {
    let temp = tempfile::tempdir().unwrap();
    let mut cmd = Command::cargo_bin("ddl").unwrap();
    common::clean_git_env(&mut cmd);
    cmd.current_dir(temp.path());
    cmd.arg("feedback")
        .arg("feature")
        .arg("--dry-run")
        .write_stdin("Add --verbose flag\nPlease add it\n");
    cmd.timeout(CMD_TIMEOUT);
    cmd.assert()
        .success()
        .stderr(predicate::str::contains("[feature] Add --verbose flag"));
}

#[test]
fn test_feedback_from_last_error_after_failing_command() {
    let temp = tempfile::tempdir().unwrap();

    // First, run a command that fails so the scratch record exists for this
    // tool + binary. HOME is isolated so the scratch lives in the temp dir.
    let mut fail = Command::cargo_bin("ddl").unwrap();
    fail.env("HOME", temp.path());
    fail.env("XDG_CACHE_HOME", temp.path().join(".cache"));
    fail.arg("install").arg("definitely-not-a-real-tool");
    fail.timeout(CMD_TIMEOUT);
    fail.assert().failure();

    let mut cmd = Command::cargo_bin("ddl").unwrap();

    common::clean_git_env(&mut cmd);
    cmd.env("HOME", temp.path());
    cmd.env("XDG_CACHE_HOME", temp.path().join(".cache"));
    cmd.current_dir(temp.path());
    cmd.arg("feedback")
        .arg("bug")
        .arg("--from-last-error")
        .arg("--dry-run");
    cmd.timeout(CMD_TIMEOUT);
    cmd.assert()
        .success()
        .stderr(predicate::str::contains("auto-reported error"))
        .stderr(predicate::str::contains("install"));
}
