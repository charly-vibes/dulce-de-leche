//! Integration tests for the `ddl setup` subcommand (DDL-g6w.1).
//!
//! Pins the wire contract before the wizard step machine exists
//! (openspec add-setup-wizard-onboarding §2.1): invoking `ddl setup`
//! must parse, dispatch to the SetupWizard entry point, and emit a JSON
//! envelope whose `data.command` is `"setup"` with flag passthrough.
//! Task §2.2 replaces the placeholder behavior; these assertions must
//! keep holding after the step machine lands.

mod common;
use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use std::time::Duration;

/// Timeout for each test.
const CMD_TIMEOUT: Duration = Duration::from_secs(20);

fn ddl_cmd() -> Command {
    let mut cmd = Command::cargo_bin("ddl").unwrap();
    common::clean_git_env(&mut cmd);
    cmd.timeout(CMD_TIMEOUT);
    cmd
}

#[test]
fn test_setup_help_shows_description() {
    ddl_cmd()
        .arg("setup")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("ddl setup").or(predicate::str::contains("setup")));
}

#[test]
fn test_setup_json_envelope_command_is_setup() {
    let out = ddl_cmd()
        .args(["setup", "--json", "-y", "--tools", "ah,spk"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let envelope: Value =
        serde_json::from_slice(&out).expect("`ddl setup --json` emits a valid JSON envelope");

    assert_eq!(envelope["ok"], serde_json::json!(true));
    assert_eq!(envelope["envelope_version"], serde_json::json!("0.1"));
    let data = &envelope["data"];
    assert_eq!(
        data["command"], "setup",
        "envelope data must identify the setup command"
    );
    assert!(
        data["status"] == "not_implemented",
        "pre-wizard placeholder must report status=not_implemented, got {data}"
    );
}

#[test]
fn test_setup_flag_passthrough() {
    let out = ddl_cmd()
        .args([
            "setup",
            "--json",
            "--yes",
            "--tools",
            "ah,specodelic",
            "--no-install",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let envelope: Value = serde_json::from_slice(&out).expect("valid JSON envelope");
    let data = &envelope["data"];

    assert_eq!(
        data["tools"],
        serde_json::json!(["ah", "specodelic"]),
        "--tools value must pass through into the envelope"
    );
    assert_eq!(data["no_install"], serde_json::json!(true));
    assert_eq!(data["yes"], serde_json::json!(true));
}

#[test]
fn test_setup_defaults_when_flags_absent() {
    let out = ddl_cmd()
        .args(["setup", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let envelope: Value = serde_json::from_slice(&out).expect("valid JSON envelope");
    let data = &envelope["data"];
    assert_eq!(data["tools"], serde_json::json!([]));
    assert_eq!(data["no_install"], serde_json::json!(false));
    assert_eq!(data["yes"], serde_json::json!(false));
}

#[test]
fn test_setup_human_mode_mentions_setup() {
    ddl_cmd()
        .args(["setup", "--human"])
        .assert()
        .success()
        .stdout(predicate::str::contains("setup"));
}
