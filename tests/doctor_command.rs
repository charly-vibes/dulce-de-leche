//! Integration tests for the `doctor` command — especially `doctor --fix`.
//!
//! Covers DDL-9ge: `doctor --fix` silently does nothing.
//! Covers DDL-1n7: `doctor` without `--fix` lists diagnostics.
mod common;

use assert_cmd::Command;
use predicates::prelude::*;
use std::time::Duration;

/// Timeout for each test.
const CMD_TIMEOUT: Duration = Duration::from_secs(60);

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
    common::clean_git_env(&mut cmd);
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
    common::clean_git_env(&mut cmd);
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
    common::clean_git_env(&mut cmd);
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
    common::clean_git_env(&mut cmd);
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
    common::clean_git_env(&mut cmd);
    cmd.current_dir(temp.path());
    cmd.arg("doctor").arg("--fix");
    cmd.timeout(CMD_TIMEOUT);
    let cmd_asserted =
        cmd.assert()
            .success()
            .stdout(predicates::prelude::predicate::str::contains(
                "manifest.json created",
            ));
    assert!(
        cmd_asserted.get_output().status.success(),
        "command should exit successfully"
    );
}

// ===================== DDL-1n7: diagnostics listing =====================

#[test]
fn test_doctor_lists_diagnostics_header() {
    let (mut cmd, _temp) = ddl_cmd();
    cmd.arg("doctor").arg("--human");
    let cmd_asserted = cmd
        .assert()
        .success()
        .stdout(predicate::str::contains("Diagnostics"));
    assert!(
        cmd_asserted.get_output().status.success(),
        "command should exit successfully"
    );
    assert!(
        String::from_utf8_lossy(&cmd_asserted.get_output().stdout).contains("Diagnostics"),
        "output should contain expected text"
    );
}

#[test]
fn test_doctor_shows_summary_line() {
    let (mut cmd, _temp) = ddl_cmd();
    cmd.arg("doctor").arg("--human");
    let cmd_asserted = cmd
        .assert()
        .success()
        .stdout(predicate::str::contains("pass:"));
    assert!(
        cmd_asserted.get_output().status.success(),
        "command should exit successfully"
    );
    assert!(
        String::from_utf8_lossy(&cmd_asserted.get_output().stdout).contains("pass:"),
        "output should contain expected text"
    );
}

#[test]
fn test_doctor_json_output() {
    let (mut cmd, _temp) = ddl_cmd();
    cmd.arg("doctor").arg("--json");
    let cmd_asserted = cmd
        .assert()
        .success()
        .stdout(predicate::str::contains("envelope_kind"))
        .stdout(predicate::str::contains("doctor"));
    assert!(
        cmd_asserted.get_output().status.success(),
        "command should exit successfully"
    );
    assert!(
        String::from_utf8_lossy(&cmd_asserted.get_output().stdout).contains("envelope_kind"),
        "output should contain expected text"
    );
    assert!(
        String::from_utf8_lossy(&cmd_asserted.get_output().stdout).contains("doctor"),
        "output should contain expected text"
    );
}

#[test]
fn test_doctor_works_without_ddl_dir() {
    // Running doctor in a dir with no .ddl/ should succeed and report it.
    let temp = tempfile::tempdir().unwrap();
    let mut cmd = Command::cargo_bin("ddl").unwrap();
    common::clean_git_env(&mut cmd);
    cmd.current_dir(temp.path());
    cmd.arg("doctor").arg("--human");
    cmd.timeout(CMD_TIMEOUT);
    let cmd_asserted = cmd.assert().success();
    assert!(
        cmd_asserted.get_output().status.success(),
        "command should exit successfully"
    );
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
    // Strip git's inherited environment: under a lefthook pre-push run the
    // child would otherwise inherit GIT_DIR/GIT_WORK_TREE from the hook and
    // `git init` would try to lock the parent repo's config instead of
    // creating a fresh repo.
    let mut cmd = std::process::Command::new("git");
    for var in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_OBJECT_DIRECTORY",
        "GIT_COMMON_DIR",
        "GIT_CONFIG",
        "GIT_CONFIG_GLOBAL",
        "GIT_CONFIG_SYSTEM",
        "GIT_NAMESPACE",
    ] {
        cmd.env_remove(var);
    }
    let out = cmd
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
        "pre-commit:\n  commands:\n{PRE_COMMIT_BLOCK}\n    bd:\n      run: bd hooks run pre-commit\n\npre-push:\n  commands:\n{PRE_PUSH_BLOCK}\n    bd:\n      run: bd hooks run pre-push\n"
    );
    std::fs::write(temp.path().join("lefthook.yml"), &lefthook).unwrap();
    let (gitignore, _) = ensure_gitignore_tool_dirs(Some(""));
    std::fs::write(temp.path().join(".gitignore"), &gitignore).unwrap();
    std::fs::write(
        temp.path().join("AGENTS.md"),
        "<!-- WAI:START -->\n<!-- WAI:END -->\n<!-- BEGIN BEADS INTEGRATION -->\n<!-- END BEADS INTEGRATION -->\n<!-- DDL:START -->\n<!-- DDL:END -->\n",
    )
    .unwrap();
    std::fs::write(
        temp.path().join("Cargo.toml"),
        "[package]\nname = \"x\"\nlicense = \"MIT\"\n",
    )
    .unwrap();
    // Fleet-standard files (DDL-rod): LICENSE matching the Cargo.toml
    // declaration, a gated fmt-check recipe, and a CHANGELOG.
    std::fs::write(
        temp.path().join("LICENSE"),
        "MIT License\n\nPermission is hereby granted, free of charge...\n",
    )
    .unwrap();
    std::fs::write(temp.path().join("CHANGELOG.md"), "# Changelog\n").unwrap();
    std::fs::write(
        temp.path().join("justfile"),
        "fmt:\n    cargo fmt\n\nfmt-check:\n    cargo fmt -- --check\n",
    )
    .unwrap();
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

    common::clean_git_env(&mut cmd);
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

    common::clean_git_env(&mut cmd);
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
        "ddl.repo.license",
        "ddl.repo.fmt",
        "ddl.repo.bd-hooks",
        "ddl.repo.changelog",
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

    common::clean_git_env(&mut cmd);
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

// ===================== agent instrumentation layouts (DDL-ehu) =====================

/// AGENTS.md carrying the blocks of the tools that still publish one. openspec
/// is deliberately absent: since 1.0.0 it publishes `openspec/config.yaml`.
const BLOCK_DOC: &str = "<!-- DDL:START -->\n<!-- DDL:END -->\n<!-- WAI:START -->\n<!-- WAI:END -->\n<!-- BEGIN BEADS INTEGRATION -->\n<!-- END BEADS INTEGRATION -->\n";

/// Run `RepoAgentBlocksCheck` directly against a fresh git repo containing
/// `files` (plus `versions.ddl.toml` when `pins` is given).
fn block_findings(
    files: &[(&str, &str)],
    pins: Option<&str>,
) -> Vec<genesis::suite_linter::LintResult> {
    let temp = tempfile::tempdir().unwrap();
    git_init(temp.path());
    for (path, contents) in files {
        let full = temp.path().join(path);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&full, contents).unwrap();
    }
    if let Some(pins) = pins {
        std::fs::write(temp.path().join("versions.ddl.toml"), pins).unwrap();
    }
    let check = dulce_de_leche::diagnostics::RepoAgentBlocksCheck;
    genesis::doctor::DoctorCheck::run(&check, temp.path()).unwrap()
}

/// Only the findings about openspec (the layout under test).
fn openspec_findings(
    files: &[(&str, &str)],
    pins: Option<&str>,
) -> Vec<genesis::suite_linter::LintResult> {
    block_findings(files, pins)
        .into_iter()
        .filter(|f| f.message.contains("openspec"))
        .collect()
}

#[test]
fn test_blocks_check_passes_openspec_config_layout() {
    let findings = openspec_findings(
        &[
            ("AGENTS.md", BLOCK_DOC),
            ("openspec/config.yaml", "schema: spec-driven\n"),
        ],
        None,
    );
    assert!(
        findings.is_empty(),
        "openspec/config.yaml is the published artifact; no finding: {findings:?}"
    );
}

#[test]
fn test_blocks_check_openspec_legacy_layout_is_a_migration_warning() {
    let findings = openspec_findings(
        &[
            ("AGENTS.md", BLOCK_DOC),
            ("openspec/project.md", "# Project Context\n"),
        ],
        None,
    );
    assert_eq!(
        findings.len(),
        1,
        "project.md alone is one migration finding: {findings:?}"
    );
    let finding = &findings[0];
    assert!(
        finding.message.contains("legacy layout")
            && finding.message.contains("openspec/config.yaml"),
        "legacy finding names the layout and the artifact: {finding:?}"
    );
    assert!(
        !finding.message.contains("managed block"),
        "openspec never requires a managed block: {finding:?}"
    );
    if let Some(fix) = finding.fix.as_deref() {
        assert!(
            fix.contains("openspec update"),
            "legacy fix is the migration command: {finding:?}"
        );
    }
    assert!(!finding.fix.as_deref().unwrap_or("").contains("ddl init"));
}

#[test]
fn test_blocks_check_openspec_hybrid_layout_names_config_as_authoritative() {
    let findings = openspec_findings(
        &[
            ("AGENTS.md", BLOCK_DOC),
            ("openspec/config.yaml", "schema: spec-driven\n"),
            ("openspec/project.md", "# Project Context\n"),
        ],
        None,
    );
    assert_eq!(
        findings.len(),
        1,
        "hybrid layout is one advisory on the stale file: {findings:?}"
    );
    let finding = &findings[0];
    assert!(
        finding.message.contains("hybrid layout")
            && finding.message.contains("authoritative")
            && finding.message.contains("project.md"),
        "hybrid finding names precedence and the stale file: {finding:?}"
    );
    if let Some(fix) = finding.fix.as_deref() {
        assert!(
            fix.contains("openspec update"),
            "hybrid fix is the migration command: {finding:?}"
        );
    }
}

#[test]
fn test_blocks_check_warns_when_scoped_openspec_published_nothing() {
    // openspec/ exists (genesis-style: used, not necessarily pinned) but
    // carries neither artifact.
    let findings = openspec_findings(
        &[("AGENTS.md", BLOCK_DOC), ("openspec/changes/.keep", "")],
        None,
    );
    assert_eq!(
        findings.len(),
        1,
        "openspec/ without an artifact is one finding: {findings:?}"
    );
    assert!(
        findings[0]
            .message
            .contains("openspec/config.yaml is missing")
    );
    assert!(!findings[0].message.contains("managed block"));
    if let Some(fix) = findings[0].fix.as_deref() {
        assert_eq!(fix, "openspec update");
    }
}

#[test]
fn test_blocks_check_skips_openspec_without_dir_or_pin() {
    let findings = block_findings(&[("AGENTS.md", BLOCK_DOC)], None);
    assert!(
        findings.is_empty(),
        "a repo that does not use openspec gets no finding: {findings:?}"
    );
}

#[test]
fn test_blocks_check_scopes_openspec_by_pin() {
    let findings = openspec_findings(
        &[("AGENTS.md", BLOCK_DOC)],
        Some("[tools]\nopenspec = \"0.19.0\"\n"),
    );
    assert_eq!(
        findings.len(),
        1,
        "a pin scopes the artifact check even without openspec/: {findings:?}"
    );
    assert!(
        findings[0]
            .message
            .contains("openspec/config.yaml is missing")
    );
}

#[test]
fn test_blocks_check_reports_retired_openspec_block_once() {
    let agents = format!("{BLOCK_DOC}<!-- OPENSPEC:START -->\n<!-- OPENSPEC:END -->\n");
    let findings = openspec_findings(&[("AGENTS.md", agents.as_str())], None);
    assert_eq!(
        findings.len(),
        1,
        "the retired block is a single advisory: {findings:?}"
    );
    let finding = &findings[0];
    assert!(
        finding.message.contains("retired") && finding.message.contains("OPENSPEC:START"),
        "advisory names the retired block: {finding:?}"
    );
    assert!(
        !finding.message.contains("missing managed block"),
        "a retired block is never demanded: {finding:?}"
    );
    if let Some(fix) = finding.fix.as_deref() {
        assert!(fix.contains("openspec update"), "{finding:?}");
    }
}

#[test]
fn test_blocks_check_missing_agents_lists_only_block_tools() {
    // openspec is scoped but publishes an artifact, so it has no claim on
    // AGENTS.md; the warning names only the block-publishing tools.
    let findings = block_findings(
        &[("openspec/config.yaml", "schema: spec-driven\n")],
        Some("[tools]\nwai = \"2026.11.11\"\nopenspec = \"0.19.0\"\n"),
    );
    assert_eq!(findings.len(), 1, "{findings:?}");
    let message = &findings[0].message;
    assert!(
        message.contains("ddl") && message.contains("wai"),
        "missing AGENTS.md lists the scoped block tools: {message}"
    );
    assert!(
        !message.contains("openspec"),
        "openspec is an artifact publisher, not a block tool: {message}"
    );
}
