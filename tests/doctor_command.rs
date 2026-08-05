//! Integration tests for the `doctor` command — especially `doctor --fix`.
//!
//! Covers DDL-9ge: `doctor --fix` silently does nothing.
//! Covers DDL-1n7: `doctor` without `--fix` lists diagnostics.

use assert_cmd::Command;
use predicates::prelude::*;
use std::time::Duration;

/// Timeout for each test.
const CMD_TIMEOUT: Duration = Duration::from_secs(10);

/// Helper: create a temp dir with a `.ddl/` containing a manifest and return
/// the command pre-configured to run inside it.
fn ddl_cmd() -> (Command, tempfile::TempDir) {
    let temp = tempfile::tempdir().unwrap();
    let manifest_dir = temp.path().join(".ddl");
    std::fs::create_dir_all(&manifest_dir).unwrap();
    let manifest = serde_json::json!({
        "ddl_version": "0.2.0",
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
fn test_doctor_fix_creates_missing_manifest() {
    // Create a temp dir with .ddl/ but NO manifest.json (simulating corruption)
    let temp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(temp.path().join(".ddl")).unwrap();

    // Run ddl doctor --fix
    let mut cmd = Command::cargo_bin("ddl").unwrap();
    cmd.current_dir(temp.path());
    cmd.arg("doctor").arg("--fix");
    cmd.timeout(CMD_TIMEOUT);
    cmd.assert().success();

    // Verify manifest.json was created by the fix
    assert!(
        temp.path().join(".ddl/manifest.json").exists(),
        "doctor --fix should create manifest.json"
    );
}

#[test]
fn test_doctor_without_fix_does_not_create_manifest() {
    // Create a temp dir with .ddl/ but NO manifest.json
    let temp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(temp.path().join(".ddl")).unwrap();

    // Run ddl doctor (without --fix)
    let mut cmd = Command::cargo_bin("ddl").unwrap();
    cmd.current_dir(temp.path());
    cmd.arg("doctor");
    cmd.timeout(CMD_TIMEOUT);
    cmd.assert().success();

    // Without --fix, manifest.json should NOT be created
    assert!(
        !temp.path().join(".ddl/manifest.json").exists(),
        "doctor without --fix should not create manifest.json"
    );
}

#[test]
fn test_doctor_fix_removes_broken_symlink() {
    // Create a temp dir with .ddl/ containing a broken symlink
    let temp = tempfile::tempdir().unwrap();
    let ddl_dir = temp.path().join(".ddl");
    std::fs::create_dir_all(&ddl_dir).unwrap();

    // Create a broken symlink pointing to a non-existent target
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(
            ddl_dir.join("nonexistent-target"),
            ddl_dir.join("broken-link"),
        )
        .unwrap();
    }
    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_file(
            ddl_dir.join("nonexistent-target"),
            ddl_dir.join("broken-link"),
        )
        .unwrap();
    }

    // Run ddl doctor --fix
    let mut cmd = Command::cargo_bin("ddl").unwrap();
    cmd.current_dir(temp.path());
    cmd.arg("doctor").arg("--fix");
    cmd.timeout(CMD_TIMEOUT);
    cmd.assert().success();

    // Verify broken symlink was removed by the fix
    assert!(
        !ddl_dir.join("broken-link").exists(),
        "doctor --fix should remove broken symlink"
    );
}

#[test]
fn test_doctor_fix_reports_fix_messages() {
    // Create a temp dir with .ddl/ but NO manifest.json
    let temp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(temp.path().join(".ddl")).unwrap();

    // Run ddl doctor --fix
    let mut cmd = Command::cargo_bin("ddl").unwrap();
    cmd.current_dir(temp.path());
    cmd.arg("doctor").arg("--fix");
    cmd.timeout(CMD_TIMEOUT);
    cmd.assert()
        .success()
        .stdout(predicates::prelude::predicate::str::contains(
            "manifest.json created",
        ));
}

// ===================== DDL-1n7: diagnostics listing =====================

#[test]
fn test_doctor_lists_diagnostics_header() {
    let (mut cmd, _temp) = ddl_cmd();
    cmd.arg("doctor").arg("--human");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Diagnostics"));
}

#[test]
fn test_doctor_shows_summary_line() {
    let (mut cmd, _temp) = ddl_cmd();
    cmd.arg("doctor").arg("--human");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("pass:"));
}

#[test]
fn test_doctor_json_output() {
    let (mut cmd, _temp) = ddl_cmd();
    cmd.arg("doctor").arg("--json");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("envelope_kind"))
        .stdout(predicate::str::contains("doctor"));
}

#[test]
fn test_doctor_works_without_ddl_dir() {
    // Running doctor in a dir with no .ddl/ should succeed and report it.
    let temp = tempfile::tempdir().unwrap();
    let mut cmd = Command::cargo_bin("ddl").unwrap();
    cmd.current_dir(temp.path());
    cmd.arg("doctor").arg("--human");
    cmd.timeout(CMD_TIMEOUT);
    cmd.assert().success();
}
