//! Characterization tests for `dot_ddl::is_git_tracked`.
//!
//! These pin the best-effort degradation contract *before* the migration
//! onto `genesis::git::tracked` (ticket genesis-tpf.7 / DDL-u8u):
//!
//! - committed path          → true
//! - untracked path          → false (not an error)
//! - path outside any repo   → false (not an error)
//!
//! The function must never panic and never return an error: symlink
//! migration treats "unknown" the same as "untracked" because untracked
//! content has no history to corrupt.
//!
//! Tests do not mutate the process environment; each case builds its own
//! throwaway git repository under a tempdir.

use std::path::{Path, PathBuf};
use std::process::Command;

use dulce_de_leche::dot_ddl::is_git_tracked;

/// Create a fresh git repository with one committed file and return
/// paths to (committed, untracked) files inside it.
fn seed_repo(dir: &Path) -> (PathBuf, PathBuf) {
    let committed = dir.join("committed.txt");
    std::fs::write(&committed, "known to git\n").expect("write committed file");
    let untracked = dir.join("untracked.txt");
    std::fs::write(&untracked, "not added\n").expect("write untracked file");

    let git = |args: &[&str]| {
        let out = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .output()
            .expect("spawn git");
        assert!(
            out.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&out.stderr)
        );
    };

    git(&["init", "--quiet"]);
    git(&["add", "committed.txt"]);
    git(&[
        "-c",
        "user.email=ddl-test@example.com",
        "-c",
        "user.name=DDL Test",
        "commit",
        "--quiet",
        "-m",
        "seed",
    ]);
    (committed, untracked)
}

#[test]
fn committed_path_is_tracked() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (committed, _untracked) = seed_repo(tmp.path());
    assert!(is_git_tracked(&committed), "committed file must be tracked");
}

#[test]
fn untracked_path_is_not_tracked() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (_committed, untracked) = seed_repo(tmp.path());
    assert!(
        !is_git_tracked(&untracked),
        "never-added file must report untracked"
    );
}

#[test]
fn path_outside_any_repo_is_not_tracked() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let bare = tmp.path().join("orphan.txt");
    std::fs::write(&bare, "no repository here\n").expect("write file");
    assert!(
        !is_git_tracked(&bare),
        "path outside a git repository must degrade to false, not error"
    );
}
