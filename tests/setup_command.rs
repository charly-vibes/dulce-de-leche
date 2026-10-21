//! Integration tests for the `ddl setup` subcommand (DDL-g6w.1, §2.2 rewire).
//!
//! DDL-g6w.8: `cmd_setup` now runs the setup_wizard step machine
//! (pre-flight → migrate → scope → delegate → verify) through the
//! production [`InitPathsDelegate`] — no behavior fork from `ddl init`
//! paths. The JSON envelope carries the per-step outcomes instead of the
//! §2.1 `status=not_implemented` placeholder, still identifies itself via
//! `data.command == "setup"`, and mirrors the flag passthrough
//! (`--tools`, `--no-install`, `-y`).
//!
//! Every machine-running test executes in a temp dir with `--no-install`
//! so no real tool installs happen; the exit-non-zero contract matches
//! `ddl init` (failures fold into the resume summary).

mod common;
use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use std::time::Duration;

/// Timeout for each test (the delegate cascade can run real tool inits).
const CMD_TIMEOUT: Duration = Duration::from_secs(60);

fn ddl_cmd() -> (Command, tempfile::TempDir) {
    let temp = tempfile::tempdir().unwrap();
    let mut cmd = Command::cargo_bin("ddl").unwrap();
    common::clean_git_env(&mut cmd);
    cmd.current_dir(temp.path());
    cmd.timeout(CMD_TIMEOUT);
    (cmd, temp)
}

/// The canonical five-step order of the wizard step machine.
const STEP_LABELS: [&str; 5] = ["pre-flight", "migrate", "scope", "delegate", "verify"];

#[test]
fn test_setup_help_shows_description() {
    Command::cargo_bin("ddl")
        .unwrap()
        .arg("setup")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("ddl setup").or(predicate::str::contains("setup")));
}

/// RED-first regression test (DDL-g6w.8): `ddl setup --json` must run the
/// step machine end-to-end — the envelope carries the five step outcomes
/// instead of the §2.1 `status=not_implemented` placeholder, still with
/// `data.command == "setup"`.
#[test]
fn test_setup_json_envelope_runs_step_machine() {
    let (mut cmd, _temp) = ddl_cmd();
    let out = cmd
        .args(["setup", "--json", "-y", "--no-install"])
        .assert()
        .get_output()
        .stdout
        .clone();
    let envelope: Value =
        serde_json::from_slice(&out).expect("`ddl setup --json` emits a valid JSON envelope");

    assert_eq!(envelope["envelope_version"], serde_json::json!("0.1"));
    let data = &envelope["data"];
    assert_eq!(
        data["command"], "setup",
        "envelope data must identify the setup command"
    );
    assert!(
        data["status"] != "not_implemented",
        "the §2.1 placeholder status must be gone, got {data}"
    );

    let steps = data["steps"]
        .as_array()
        .expect("envelope data carries the per-step outcomes");
    let labels: Vec<&str> = steps
        .iter()
        .map(|s| s["step"].as_str().expect("step label"))
        .collect();
    assert_eq!(
        labels, STEP_LABELS,
        "the five wizard steps must run in canonical order"
    );
    for s in steps {
        assert!(
            s["status"] == "ok" || s["status"] == "failed",
            "each step reports ok|failed, got {s}"
        );
    }
    assert!(
        data["ok"].is_boolean(),
        "envelope data carries the machine verdict, got {data}"
    );
    assert!(
        data["resume_summary"].is_array(),
        "envelope data carries the resume summary, got {data}"
    );
}

/// Step-machine failures keep init's exit-non-zero contract: a fresh temp
/// dir has nothing installed, so verify reports unhealthy items and the
/// run must exit non-zero with the verdict in the envelope.
#[test]
fn test_setup_failure_exits_non_zero_with_summary() {
    let (mut cmd, _temp) = ddl_cmd();
    let out = cmd
        .args(["setup", "--json", "-y", "--no-install"])
        .assert()
        .failure()
        .get_output()
        .stdout
        .clone();
    let envelope: Value = serde_json::from_slice(&out).expect("valid JSON envelope");
    let data = &envelope["data"];
    assert_eq!(
        data["ok"],
        serde_json::json!(false),
        "a fresh temp dir cannot verify healthy — machine verdict is false"
    );
    let summary = data["resume_summary"]
        .as_array()
        .expect("resume summary array");
    assert!(
        !summary.is_empty(),
        "failures fold into the resume summary, got {data}"
    );
}

#[test]
fn test_setup_flag_passthrough() {
    let (mut cmd, _temp) = ddl_cmd();
    let out = cmd
        .args([
            "setup",
            "--json",
            "--yes",
            "--tools",
            "ah,specodelic",
            "--no-install",
        ])
        .assert()
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
fn test_setup_tools_default_to_empty() {
    let (mut cmd, _temp) = ddl_cmd();
    let out = cmd
        .args(["setup", "--json", "-y", "--no-install"])
        .assert()
        .get_output()
        .stdout
        .clone();
    let envelope: Value = serde_json::from_slice(&out).expect("valid JSON envelope");
    let data = &envelope["data"];
    assert_eq!(
        data["tools"],
        serde_json::json!([]),
        "absent --tools parses to an empty tool list"
    );
}

#[test]
fn test_setup_human_mode_mentions_setup_and_steps() {
    let (mut cmd, _temp) = ddl_cmd();
    cmd.args(["setup", "--human", "--no-install", "-y"])
        .assert()
        .stdout(predicate::str::contains("setup"))
        .stdout(predicate::str::contains("pre-flight"))
        .stdout(predicate::str::contains("verify"));
}
