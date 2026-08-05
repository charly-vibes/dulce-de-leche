//! Integration tests for the `doctor` command — especially `doctor --fix`.
//!
//! Covers DDL-9ge: `doctor --fix` silently does nothing.

use assert_cmd::Command;
use std::time::Duration;

/// Timeout for each test.
const CMD_TIMEOUT: Duration = Duration::from_secs(10);

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
        std::os::unix::fs::symlink(ddl_dir.join("nonexistent-target"), ddl_dir.join("broken-link"))
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
    cmd.assert().success().stdout(
        predicates::prelude::predicate::str::contains("manifest.json created"),
    );
}
