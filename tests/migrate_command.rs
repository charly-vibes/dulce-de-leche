//! Integration tests for the `ddl migrate` command (DDL-6zn.7).
//!
//! Covers the gh#30 bug class: phase-1 symlink migration must be safe on
//! git-tracked config dirs (refuse/skip with guidance), and `--undo` must
//! clear all manifest residue (`migration_state` back to `none`).
mod common;

use assert_cmd::Command;
use predicates::prelude::*;
use std::path::Path;
use std::process::Command as StdCommand;
use std::time::Duration;

/// Timeout for each test.
const CMD_TIMEOUT: Duration = Duration::from_secs(10);

/// Helper: a `ddl` command rooted at a fresh temp dir with a `.ddl/` manifest.
fn ddl_cmd() -> (Command, tempfile::TempDir) {
    let temp = tempfile::tempdir().unwrap();
    let manifest_dir = temp.path().join(".ddl");
    std::fs::create_dir_all(&manifest_dir).unwrap();
    let manifest = serde_json::json!({
        "ddl_version": "0.7.0",
        "migration_state": "none",
        "tools": {}
    });
    std::fs::write(
        manifest_dir.join("manifest.json"),
        serde_json::to_string_pretty(&manifest).unwrap(),
    )
    .unwrap();
    let mut cmd = Command::cargo_bin("ddl").unwrap();
    common::clean_git_env(&mut cmd);
    cmd.current_dir(temp.path());
    cmd.timeout(CMD_TIMEOUT);
    (cmd, temp)
}

/// Helper: run git non-interactively in the given dir; panics on failure.
fn git(dir: &Path, args: &[&str]) {
    let out = StdCommand::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@t")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@t")
        .output()
        .expect("run git");
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

// ===== gh#30: migrate must be safe on git-tracked config dirs =====

#[test]
fn test_migrate_skips_git_tracked_legacy_dir() {
    let (mut cmd, temp) = ddl_cmd();

    // A git repo with a tracked .wai/ config dir
    let wai = temp.path().join(".wai");
    std::fs::create_dir_all(&wai).unwrap();
    std::fs::write(wai.join("resources.toml"), "shared = true").unwrap();
    git(temp.path(), &["init", "-q"]);
    git(temp.path(), &["add", ".wai/resources.toml"]);
    git(temp.path(), &["commit", "-q", "-m", "track wai config"]);

    cmd.arg("migrate");
    cmd.assert().success();

    // The tracked dir must NOT be replaced by a symlink
    let meta = std::fs::symlink_metadata(&wai).unwrap();
    assert!(
        !meta.file_type().is_symlink(),
        ".wai must not become a symlink when git-tracked"
    );
    assert!(
        wai.join("resources.toml").exists(),
        "tracked file must stay in place"
    );

    // Guidance must be shown
    cmd.assert()
        .stdout(predicates::str::contains("git").or(predicates::str::contains("skipped")));
}

#[test]
fn test_migrate_still_migrates_untracked_dirs_alongside_tracked() {
    let (mut cmd, temp) = ddl_cmd();

    // Tracked .wai/, untracked .testaruda/
    let wai = temp.path().join(".wai");
    std::fs::create_dir_all(&wai).unwrap();
    std::fs::write(wai.join("resources.toml"), "shared = true").unwrap();
    let testaruda = temp.path().join(".testaruda");
    std::fs::create_dir_all(&testaruda).unwrap();
    std::fs::write(testaruda.join("config.toml"), "tuned = true").unwrap();
    git(temp.path(), &["init", "-q"]);
    git(temp.path(), &["add", ".wai/resources.toml"]);
    git(temp.path(), &["commit", "-q", "-m", "track wai config"]);

    cmd.arg("migrate");
    cmd.assert().success();

    // Tracked: untouched. Untracked: migrated (symlinked).
    assert!(
        !std::fs::symlink_metadata(&wai)
            .unwrap()
            .file_type()
            .is_symlink(),
        "tracked .wai must not be migrated"
    );
    assert!(
        std::fs::symlink_metadata(&testaruda)
            .unwrap()
            .file_type()
            .is_symlink(),
        "untracked .testaruda should be migrated"
    );
}

// ===== gh#30 residue: --undo must clear manifest migration_state =====

#[test]
fn test_migrate_undo_resets_migration_state_in_manifest() {
    let (mut cmd, temp) = ddl_cmd();

    // Simulate a migrated repo: .ddl/wai holds the config, .wai is the symlink
    let tool_dir = temp.path().join(".ddl/wai");
    std::fs::create_dir_all(&tool_dir).unwrap();
    std::fs::write(tool_dir.join("config.toml"), "key = true").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(".ddl/wai", temp.path().join(".wai")).unwrap();
    #[cfg(windows)]
    std::os::windows::fs::symlink_dir(".ddl/wai", temp.path().join(".wai")).unwrap();

    let manifest_path = temp.path().join(".ddl/manifest.json");
    let manifest = serde_json::json!({
        "ddl_version": "0.7.0",
        "migration_state": "phase1",
        "tools": {}
    });
    std::fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&manifest).unwrap(),
    )
    .unwrap();

    cmd.arg("migrate").arg("--undo");
    cmd.assert().success();

    // Residue check: migration_state must be back to none
    let after = std::fs::read_to_string(&manifest_path).unwrap();
    assert!(
        after.contains("\"none\""),
        "migration_state must reset to none after --undo, got: {after}"
    );
    assert!(
        !after.contains("phase1"),
        "no phase1 residue may remain in the manifest, got: {after}"
    );

    // And the legacy config is a real dir again
    assert!(temp.path().join(".wai/config.toml").exists());
}
