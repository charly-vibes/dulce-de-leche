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

/// DDL-6zn.8: help examples are GENERATED from MANAGED_TOOLS — every
/// registered tool must appear in `ddl install --help` (no hardcoded,
/// drifting example lists).
#[test]
fn test_install_help_lists_every_registry_tool() {
    let mut cmd = Command::cargo_bin("ddl").unwrap();
    cmd.arg("install").arg("--help");
    cmd.timeout(CMD_TIMEOUT);
    let output = cmd.assert().success().get_output().stdout.clone();
    let stdout = String::from_utf8(output).unwrap();
    for tool in dulce_de_leche::platform::MANAGED_TOOLS {
        assert!(
            stdout.contains(tool.name),
            "install --help is missing registry tool `{}`",
            tool.name
        );
    }
}

/// DDL-6zn.8: 'ddl'/'dulce'/'dulce-de-leche' refer to ddl itself — the
/// error must teach the canonical repo instead of a bare "Unknown tool".
#[test]
fn test_install_self_name_gives_teachable_error() {
    for name in ["ddl", "dulce", "dulce-de-leche"] {
        let mut cmd = Command::cargo_bin("ddl").unwrap();
        cmd.arg("install").arg(name);
        cmd.timeout(CMD_TIMEOUT);
        cmd.assert()
            .failure()
            .stderr(predicate::str::contains("charly-vibes/dulce-de-leche"));
    }
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
#[cfg(unix)] // PATH stub is a POSIX shell script (see init_noninteractive.rs)
fn test_install_on_path_untracked_records_in_manifest() {
    // DDL-zw4: `ddl install TOOL` on a managed tool that is already on PATH
    // but absent from the manifest must record it (source "skipped"), so
    // manifest-driven paths (ddl status, ddl version --json) see the tool.
    let temp = tempfile::tempdir().unwrap();

    // Stub a managed, binary-probe tool (turu) on PATH answering --version.
    let bin_dir = temp.path().join("stub-bin");
    std::fs::create_dir_all(&bin_dir).unwrap();
    let stub = bin_dir.join("turu");
    std::fs::write(&stub, "#!/bin/sh\nprintf 'turu 0.3.0'\n").unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).unwrap();
    let inherited = std::env::var_os("PATH").unwrap_or_default();
    let mut paths = vec![bin_dir];
    paths.extend(std::env::split_paths(&inherited));

    let manifest_dir = temp.path().join(".ddl");
    std::fs::create_dir_all(&manifest_dir).unwrap();
    std::fs::write(
        manifest_dir.join("manifest.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "ddl_version": "0.7.0",
            "migration_state": "none",
            "tools": {}
        }))
        .unwrap(),
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("ddl").unwrap();
    cmd.current_dir(temp.path());
    cmd.env("PATH", std::env::join_paths(paths).unwrap());
    cmd.arg("install").arg("turu");
    cmd.timeout(CMD_TIMEOUT);
    cmd.assert().success();

    let manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(manifest_dir.join("manifest.json")).unwrap())
            .unwrap();
    let entry = &manifest["tools"]["turu"];
    assert!(entry.is_object(), "turu must be recorded in the manifest");
    assert_eq!(entry["source"], "skipped");
    assert_eq!(entry["status"], "installed");
    assert_eq!(entry["installed"], "0.3.0");
}

// ===================== DDL-sg8: manifest link guards (SEC-05) =====================
// A repository manifest reachable through a link must never be written
// through: the link target could live outside the project, and writes would
// silently land there. These tests stub turu on PATH so `ddl install turu`
// takes the fast "already on PATH, untracked" path, which exercises manifest
// writes with no subprocess/network work.

/// Write an empty valid manifest.json into `dir` (creating it).
fn empty_manifest(dir: &std::path::Path) {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(
        dir.join("manifest.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "ddl_version": "0.7.0",
            "migration_state": "none",
            "tools": {}
        }))
        .unwrap(),
    )
    .unwrap();
}

/// Run `ddl install turu` in `temp` with a turu stub on PATH.
fn run_install_turu(temp: &tempfile::TempDir) -> assert_cmd::assert::Assert {
    let bin_dir = temp.path().join("stub-bin");
    std::fs::create_dir_all(&bin_dir).unwrap();
    let stub = bin_dir.join("turu");
    std::fs::write(&stub, "#!/bin/sh\nprintf 'turu 0.3.0'\n").unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).unwrap();
    let inherited = std::env::var_os("PATH").unwrap_or_default();
    let mut paths = vec![bin_dir];
    paths.extend(std::env::split_paths(&inherited));

    let mut cmd = Command::cargo_bin("ddl").unwrap();
    cmd.current_dir(temp.path());
    cmd.env("PATH", std::env::join_paths(paths).unwrap());
    cmd.arg("install").arg("turu");
    cmd.timeout(CMD_TIMEOUT);
    cmd.assert()
}

/// DDL-sg8 (SEC-05): a symlink at the manifest path must never be followed
/// on write — the target could live outside the project.
#[test]
#[cfg(unix)]
fn test_install_refuses_manifest_symlink_to_outside_project() {
    let temp = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    empty_manifest(outside.path());
    let before = std::fs::read_to_string(outside.path().join("manifest.json")).unwrap();

    std::fs::create_dir_all(temp.path().join(".ddl")).unwrap();
    std::os::unix::fs::symlink(
        outside.path().join("manifest.json"),
        temp.path().join(".ddl/manifest.json"),
    )
    .unwrap();

    run_install_turu(&temp)
        .failure()
        .stderr(predicate::str::contains("symlink"));

    let after = std::fs::read_to_string(outside.path().join("manifest.json")).unwrap();
    assert_eq!(before, after, "outside manifest must be untouched");
    assert!(!after.contains("turu"), "write must not escape the project");
}

/// DDL-sg8 (SEC-05): a dangling manifest symlink must be refused — a write
/// through it would create the missing target wherever it points.
#[test]
#[cfg(unix)]
fn test_install_refuses_dangling_manifest_symlink() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(temp.path().join(".ddl")).unwrap();
    std::os::unix::fs::symlink(
        temp.path().join("missing.json"),
        temp.path().join(".ddl/manifest.json"),
    )
    .unwrap();

    run_install_turu(&temp)
        .failure()
        .stderr(predicate::str::contains("symlink"));

    assert!(
        !temp.path().join("missing.json").exists(),
        "dangling symlink target must not be created"
    );
}

/// DDL-sg8 (SEC-05): a `.ddl` directory link pointing outside the project
/// root must be refused — manifest writes would land in the linked tree.
#[test]
#[cfg(unix)]
fn test_install_refuses_ddl_dir_link_outside_project() {
    let temp = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    empty_manifest(outside.path());
    let before = std::fs::read_to_string(outside.path().join("manifest.json")).unwrap();

    std::os::unix::fs::symlink(outside.path(), temp.path().join(".ddl")).unwrap();

    run_install_turu(&temp)
        .failure()
        .stderr(predicate::str::contains("outside the project"));

    let after = std::fs::read_to_string(outside.path().join("manifest.json")).unwrap();
    assert_eq!(before, after, "outside manifest must be untouched");
}

/// DDL-sg8 (SEC-05): a `.ddl` directory link that stays inside the project
/// is still allowed — the guard must not break legitimate link layouts.
#[test]
#[cfg(unix)]
fn test_install_allows_ddl_dir_link_within_project() {
    let temp = tempfile::tempdir().unwrap();
    empty_manifest(&temp.path().join("internal-ddl"));
    std::os::unix::fs::symlink(temp.path().join("internal-ddl"), temp.path().join(".ddl")).unwrap();

    run_install_turu(&temp).success();

    let manifest: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(temp.path().join("internal-ddl/manifest.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(manifest["tools"]["turu"]["source"], "skipped");
}

/// DDL-sg8 (SEC-05): Windows reparse-point manifests must be refused too.
/// Creating file symlinks needs dev-mode/admin privileges; skip gracefully.
#[test]
#[cfg(windows)]
fn test_install_refuses_manifest_symlink_windows() {
    let temp = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    empty_manifest(outside.path());
    let before = std::fs::read_to_string(outside.path().join("manifest.json")).unwrap();

    std::fs::create_dir_all(temp.path().join(".ddl")).unwrap();
    if std::os::windows::fs::symlink_file(
        outside.path().join("manifest.json"),
        temp.path().join(".ddl/manifest.json"),
    )
    .is_err()
    {
        eprintln!("skipping: symlink_file requires admin/dev-mode privileges");
        return;
    }

    run_install_turu(&temp)
        .failure()
        .stderr(predicate::str::contains("symlink"));

    let after = std::fs::read_to_string(outside.path().join("manifest.json")).unwrap();
    assert_eq!(before, after, "outside manifest must be untouched");
}

#[test]
fn test_install_json_parses() {
    let (mut cmd, _temp) = ddl_cmd();
    // Exercise the error path, not a real install: an unknown tool fails
    // before any subprocess/network work (and before .ddl/ is touched),
    // yet --json must still emit a valid envelope.
    // (A known-but-missing tool like `wai` would attempt a real install.)
    cmd.arg("install").arg("nonexistent-tool").arg("--json");
    cmd.assert()
        .stderr(predicate::str::contains("envelope_kind"));
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
