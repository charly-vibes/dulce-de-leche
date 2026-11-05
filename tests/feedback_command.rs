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
    let cmd_asserted = cmd
        .assert()
        .success()
        .stdout(predicate::str::contains("bug"))
        .stdout(predicate::str::contains("feature"));
    assert!(
        cmd_asserted.get_output().status.success(),
        "command should exit successfully"
    );
    assert!(
        String::from_utf8_lossy(&cmd_asserted.get_output().stdout).contains("bug"),
        "output should contain expected text"
    );
    assert!(
        String::from_utf8_lossy(&cmd_asserted.get_output().stdout).contains("feature"),
        "output should contain expected text"
    );
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
    let cmd_asserted = cmd
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown kind"));
    assert!(
        cmd_asserted.get_output().status.code() != Some(0),
        "command should fail"
    );
    assert!(
        String::from_utf8_lossy(&cmd_asserted.get_output().stderr).contains("unknown kind"),
        "output should contain expected text"
    );
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
    let cmd_asserted = cmd
        .assert()
        .failure()
        .stderr(predicate::str::contains("No issue content specified"));
    assert!(
        cmd_asserted.get_output().status.code() != Some(0),
        "command should fail"
    );
    assert!(
        String::from_utf8_lossy(&cmd_asserted.get_output().stderr)
            .contains("No issue content specified"),
        "output should contain expected text"
    );
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
    let cmd_asserted = cmd
        .assert()
        .success()
        .stderr(predicate::str::contains("[bug] Crash on init"))
        .stderr(predicate::str::contains("## Description"))
        .stderr(predicate::str::contains(
            "Would file: gh issue create --repo charly-vibes/dulce-de-leche",
        ));
    assert!(
        cmd_asserted.get_output().status.success(),
        "command should exit successfully"
    );
    assert!(
        String::from_utf8_lossy(&cmd_asserted.get_output().stderr).contains("[bug] Crash on init"),
        "output should contain expected text"
    );
    assert!(
        String::from_utf8_lossy(&cmd_asserted.get_output().stderr).contains("## Description"),
        "output should contain expected text"
    );
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
    let cmd_asserted = cmd
        .assert()
        .success()
        .stderr(predicate::str::contains("[feature] Add --verbose flag"));
    assert!(
        cmd_asserted.get_output().status.success(),
        "command should exit successfully"
    );
    assert!(
        String::from_utf8_lossy(&cmd_asserted.get_output().stderr)
            .contains("[feature] Add --verbose flag"),
        "output should contain expected text"
    );
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
    let fail_asserted = fail.assert().failure();
    assert!(
        fail_asserted.get_output().status.code() != Some(0),
        "command should fail"
    );

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
    let cmd_asserted = cmd
        .assert()
        .success()
        .stderr(predicate::str::contains("auto-reported error"))
        .stderr(predicate::str::contains("install"));
    assert!(
        cmd_asserted.get_output().status.success(),
        "command should exit successfully"
    );
    assert!(
        String::from_utf8_lossy(&cmd_asserted.get_output().stderr).contains("auto-reported error"),
        "output should contain expected text"
    );
    assert!(
        String::from_utf8_lossy(&cmd_asserted.get_output().stderr).contains("install"),
        "output should contain expected text"
    );
}
