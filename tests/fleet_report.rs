//! Fleet close-loop command tests (DDL-9kz.6 `ddl gates run`; DDL-9kz.5
//! `ddl fleet report` golden tests join later).
//!
//! Responsibilities:
//! - pin the timeout-immune gate runner: detached child survives the
//!   caller, wait mode streams and exits with the child's status
//! - JSON consumers get the handle in the envelope, no interleaved output
//! - no session content: these tests only exercise process handles

use assert_cmd::Command;
use serde_json::Value;
use std::time::{Duration, Instant};

const CMD_TIMEOUT: Duration = Duration::from_secs(60);

fn gates_run(root: &std::path::Path, args: &[&str]) -> Command {
    let mut cmd = Command::cargo_bin("ddl").expect("gates run test fixture");
    cmd.current_dir(root);
    for a in args {
        cmd.arg(a);
    }
    cmd.timeout(CMD_TIMEOUT);
    cmd
}

/// Human wait mode: the gate runs, ddl streams it, and ddl exits with
/// the child's status (acceptance: `ddl gates run -- just ci` exits with
/// just ci's status).
#[test]
fn gates_run_waits_and_exits_with_child_status() {
    let temp = tempfile::tempdir().expect("gates run test fixture");
    let root = temp.path();
    std::fs::create_dir_all(root.join(".ddl/gates")).expect("gates run test fixture");

    gates_run(
        root,
        &[
            "--human",
            "gates",
            "run",
            "--wait-secs",
            "20",
            "--",
            "sh",
            "-c",
            "echo streamed-line; exit 3",
        ],
    )
    .assert()
    .failure()
    .code(3)
    .stdout(predicates::str::contains("streamed-line"))
    .stderr(predicates::str::contains("gate failed with exit 3"));

    // The status file records the child's exit for post-hoc reads.
    let dir = std::fs::read_dir(root.join(".ddl/gates")).expect("gates run test fixture");
    let statuses: Vec<_> = dir
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|x| x == "status"))
        .collect();
    assert_eq!(statuses.len(), 1, "one status file per run");
    assert!(
        std::fs::read_to_string(statuses[0].path())
            .expect("gates run test fixture")
            .contains("3"),
        "status file carries the child exit code"
    );
}

/// JSON wait mode: the gate's exit code travels in the envelope AND the
/// process exit status; no raw gate output interleaves the JSON.
#[test]
fn gates_run_json_exits_with_child_status() {
    let temp = tempfile::tempdir().expect("gates run test fixture");
    let root = temp.path();
    std::fs::create_dir_all(root.join(".ddl/gates")).expect("gates run test fixture");

    let out = gates_run(
        root,
        &[
            "gates",
            "run",
            "--wait-secs",
            "20",
            "--",
            "sh",
            "-c",
            "exit 7",
        ],
    )
    .assert()
    .failure()
    .code(7)
    .get_output()
    .stdout
    .clone();
    let envelope: Value = serde_json::from_slice(&out).expect("valid JSON envelope");
    assert_eq!(envelope["ok"], true);
    assert_eq!(envelope["data"]["exit_code"], 7);
    assert_eq!(envelope["data"]["status"], "exited(7)");
}

/// Detach mode: returns immediately with a handle (pid + log); the child
/// outlives the caller.
#[test]
fn gates_run_detach_returns_handle_and_child_survives() {
    let temp = tempfile::tempdir().expect("gates run test fixture");
    let root = temp.path();
    std::fs::create_dir_all(root.join(".ddl/gates")).expect("gates run test fixture");

    let started = Instant::now();
    let out = gates_run(root, &["gates", "run", "--detach", "--", "sleep", "3"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "detach must return immediately"
    );

    let envelope: Value = serde_json::from_slice(&out).expect("detach prints a JSON envelope");
    let handle = &envelope["data"];
    assert_eq!(handle["status"], "running");
    let pid = handle["pid"].as_u64().expect("pid in handle");
    assert!(pid > 0);

    // The child is still alive after the caller exited.
    std::thread::sleep(Duration::from_millis(500));
    assert!(
        std::path::Path::new(&format!("/proc/{pid}")).exists(),
        "detached child must survive the caller"
    );
    // Cleanup: no orphaned processes.
    let _ = std::process::Command::new("kill")
        .arg(pid.to_string())
        .output();
}

/// Long gate under wait mode: ddl degrades to the handle instead of being
/// killed by the harness timeout — the child keeps running detached.
#[test]
fn gates_run_releases_to_handle_on_long_gate() {
    let temp = tempfile::tempdir().expect("gates run test fixture");
    let root = temp.path();
    std::fs::create_dir_all(root.join(".ddl/gates")).expect("gates run test fixture");

    let out = gates_run(
        root,
        &["gates", "run", "--wait-secs", "1", "--", "sleep", "60"],
    )
    .assert()
    .success()
    .get_output()
    .stdout
    .clone();
    let envelope: Value = serde_json::from_slice(&out).expect("long gate prints the envelope");
    let handle = &envelope["data"];
    assert_eq!(handle["status"], "running");
    assert!(envelope["data"]["exit_code"].is_null(), "no exit yet");
    let pid = handle["pid"].as_u64().expect("gates run test fixture");

    assert!(
        std::path::Path::new(&format!("/proc/{pid}")).exists(),
        "child keeps running after ddl released"
    );
    let _ = std::process::Command::new("kill")
        .arg(pid.to_string())
        .output();
}

/// A live run refuses a second concurrent gate; the running handle is the
/// evidence, not a second process.
#[test]
fn gates_run_refuses_concurrent_gate() {
    let temp = tempfile::tempdir().expect("gates run test fixture");
    let root = temp.path();
    std::fs::create_dir_all(root.join(".ddl/gates")).expect("gates run test fixture");

    let out = gates_run(root, &["gates", "run", "--detach", "--", "sleep", "30"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let envelope: Value = serde_json::from_slice(&out).expect("gates run test fixture");
    let pid = envelope["data"]["pid"]
        .as_u64()
        .expect("gates run test fixture");

    gates_run(root, &["gates", "run", "--detach", "--", "true"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("already running"));
    // The refused spawn must not have replaced the live handle.
    let after =
        std::fs::read_to_string(root.join(".ddl/gates/run.json")).expect("gates run test fixture");
    assert!(
        after.contains(&format!("\"pid\": {pid}")),
        "live handle must survive a refused spawn: {after}"
    );

    let _ = std::process::Command::new("kill")
        .arg(pid.to_string())
        .output();
}
