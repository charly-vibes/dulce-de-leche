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

// ===================== DDL-6zn.5: repo-aware doctor =====================

/// Helper: parse the doctor JSON envelope's diagnostics array.
fn doctor_diagnostics(cmd: &mut Command) -> Vec<serde_json::Value> {
    let out = cmd.assert().success().get_output().stdout.clone();
    let envelope: serde_json::Value = serde_json::from_slice(&out).unwrap();
    envelope["data"]["diagnostics"]
        .as_array()
        .cloned()
        .unwrap_or_default()
}

fn repo_items(items: &[serde_json::Value]) -> Vec<&serde_json::Value> {
    items
        .iter()
        .filter(|i| {
            i["check"]
                .as_str()
                .unwrap_or_default()
                .starts_with("ddl.repo.")
        })
        .collect()
}

#[test]
fn test_doctor_repo_checks_absent_outside_git_repo() {
    // In a plain temp dir (no .git) the repo-scope section must not fire —
    // repo conformance is only meaningful inside a repository.
    let (mut cmd, _temp) = ddl_cmd();
    cmd.args(["doctor", "--json"]);
    let items = doctor_diagnostics(&mut cmd);
    assert!(
        repo_items(&items).is_empty(),
        "repo checks must be skipped outside a git repo: {:?}",
        repo_items(&items)
    );
}

/// Wire a git repo fully: managed lefthook blocks, AGENTS.md blocks,
/// gitignore coverage, version pins. Returns the temp dir.
fn git_init(path: &std::path::Path) {
    let out = std::process::Command::new("git")
        .args(["init", "-q"])
        .current_dir(path)
        .output()
        .expect("git init runs");
    assert!(out.status.success(), "git init failed: {:?}", out.stderr);
}

fn wired_git_repo() -> tempfile::TempDir {
    use dulce_de_leche::gates::{PRE_COMMIT_BLOCK, PRE_PUSH_BLOCK, ensure_gitignore_tool_dirs};
    let temp = tempfile::tempdir().unwrap();
    git_init(temp.path());
    let lefthook = format!(
        "pre-commit:\n  commands:\n{PRE_COMMIT_BLOCK}\n\npre-push:\n  commands:\n{PRE_PUSH_BLOCK}\n"
    );
    std::fs::write(temp.path().join("lefthook.yml"), &lefthook).unwrap();
    let (gitignore, _) = ensure_gitignore_tool_dirs(Some(""));
    std::fs::write(temp.path().join(".gitignore"), &gitignore).unwrap();
    std::fs::write(
        temp.path().join("AGENTS.md"),
        "<!-- WAI:START -->\n<!-- WAI:END -->\n<!-- OPENSPEC:START -->\n<!-- OPENSPEC:END -->\n<!-- BEGIN BEADS INTEGRATION -->\n<!-- END BEADS INTEGRATION -->\n",
    )
    .unwrap();
    std::fs::write(temp.path().join("Cargo.toml"), "[package]\nname = \"x\"\n").unwrap();
    std::fs::write(
        temp.path().join("versions.ddl.toml"),
        "[tools]\nwai = \"latest\"\n",
    )
    .unwrap();
    temp
}

#[test]
fn test_doctor_repo_checks_found_from_subdirectory() {
    // The repo-scope checks walk up to the repo root — doctor run from a
    // nested directory still answers "is THIS repo initialized?".
    let temp = wired_git_repo();
    std::fs::create_dir_all(temp.path().join("src/deep/nested")).unwrap();

    let mut cmd = Command::cargo_bin("ddl").unwrap();
    cmd.current_dir(temp.path().join("src/deep/nested"));
    cmd.timeout(CMD_TIMEOUT);
    cmd.args(["doctor", "--json"]);
    let items = doctor_diagnostics(&mut cmd);
    let repo = repo_items(&items);
    assert!(
        repo.iter()
            .any(|i| i["check"] == serde_json::json!("ddl.repo.gates")),
        "gates check must fire from a subdirectory of a git repo"
    );
    for item in &repo {
        assert_eq!(
            item["level"],
            serde_json::json!("pass"),
            "wired repo passes: {item}"
        );
    }
}

#[test]
fn test_doctor_repo_checks_pass_in_fully_wired_repo() {
    let temp = wired_git_repo();
    let manifest_dir = temp.path().join(".ddl");
    std::fs::create_dir_all(&manifest_dir).unwrap();
    std::fs::write(
        manifest_dir.join("manifest.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "ddl_version": "0.3.0",
            "migration_state": "none",
            "tools": {}
        }))
        .unwrap(),
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("ddl").unwrap();
    cmd.current_dir(temp.path());
    cmd.timeout(CMD_TIMEOUT);
    cmd.args(["doctor", "--json"]);
    let items = doctor_diagnostics(&mut cmd);
    let repo = repo_items(&items);
    // All four repo-scope checks present and attributed to ddl
    let checks: Vec<&str> = repo.iter().map(|i| i["check"].as_str().unwrap()).collect();
    for expected in [
        "ddl.repo.gates",
        "ddl.repo.blocks",
        "ddl.repo.gitignore",
        "ddl.repo.pins",
    ] {
        assert!(
            checks.contains(&expected),
            "missing repo check '{expected}' in {checks:?}"
        );
    }
    for item in &repo {
        assert_eq!(
            item["tool"],
            serde_json::json!("ddl"),
            "repo checks attribute to ddl"
        );
        assert_eq!(
            item["level"],
            serde_json::json!("pass"),
            "wired repo passes: {item}"
        );
    }
}

#[test]
fn test_doctor_repo_checks_warn_in_unwired_git_repo() {
    let temp = tempfile::tempdir().unwrap();
    git_init(temp.path());

    let mut cmd = Command::cargo_bin("ddl").unwrap();
    cmd.current_dir(temp.path());
    cmd.timeout(CMD_TIMEOUT);
    cmd.args(["doctor", "--json"]);
    let items = doctor_diagnostics(&mut cmd);
    let repo = repo_items(&items);
    assert!(
        !repo.is_empty(),
        "unwired git repo must produce repo findings"
    );
    let gates = repo
        .iter()
        .find(|i| i["check"] == serde_json::json!("ddl.repo.gates"))
        .expect("gates finding");
    assert_eq!(gates["level"], serde_json::json!("warn"));
    assert!(
        gates["fix"]
            .as_str()
            .unwrap_or("")
            .contains("ddl init --gates"),
        "gates findings carry the fix command: {gates}"
    );
}
