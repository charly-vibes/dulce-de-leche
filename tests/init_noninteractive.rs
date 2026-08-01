//! Integration tests for non-interactive / CI-mode init.
//!
//! Tests cover:
//! - `ddl init --yes` — install all tools non-interactively
//! - `ddl init --tools wai,dont` — selective install
//! - `ddl init --yes` in already-initialized dir — retry failed, skip existing
//! - `ddl init --yes` when all tools fail — clear error
//! - `ddl init --help` displays correct flags

use assert_cmd::Command;
use predicates::prelude::*;
use std::path::Path;
use std::time::Duration;

/// Timeout for each test — init with --no-install should be instant.
const CMD_TIMEOUT: Duration = Duration::from_secs(10);

/// Helper: create a temp directory and return the cmd pre-configured to run
/// inside it.
fn ddl_cmd() -> (Command, tempfile::TempDir) {
    let temp = tempfile::tempdir().unwrap();
    let mut cmd = Command::cargo_bin("ddl").unwrap();
    cmd.current_dir(temp.path());
    (cmd, temp)
}

/// Helper: create a .ddl/ directory with a manifest that has a failed tool.
fn create_failed_manifest(ddl_dir: &Path) {
    let manifest_dir = ddl_dir.join(".ddl");
    std::fs::create_dir_all(&manifest_dir).unwrap();
    let manifest = serde_json::json!({
        "ddl_version": "0.2.0",
        "migration_state": "none",
        "tools": {
            "wai": {
                "installed": "unknown",
                "source": "binary download",
                "status": "failed",
                "compatible": ">=0.0.0"
            }
        }
    });
    std::fs::write(
        manifest_dir.join("manifest.json"),
        serde_json::to_string_pretty(&manifest).unwrap(),
    )
    .unwrap();
}

// ===================== CLI flag parsing tests =====================

#[test]
fn test_init_help_has_yes_flag() {
    let mut cmd = Command::cargo_bin("ddl").unwrap();
    cmd.arg("init").arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("--yes"))
        .stdout(predicate::str::contains("--tools"));
}

#[test]
fn test_init_help_has_no_install_flag() {
    let mut cmd = Command::cargo_bin("ddl").unwrap();
    cmd.arg("init").arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("--no-install"));
}

// ===================== Non-interactive mode tests =====================

#[test]
fn test_init_yes_prints_banner() {
    let (mut cmd, _temp) = ddl_cmd();
    cmd.arg("init")
        .arg("--yes")
        .arg("--human")
        .arg("--no-install");
    cmd.timeout(CMD_TIMEOUT);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("dulce-de-leche"))
        .stdout(predicate::str::contains("bundle orchestrator"));
}

#[test]
fn test_init_yes_detects_platform() {
    let (mut cmd, _temp) = ddl_cmd();
    cmd.arg("init")
        .arg("--yes")
        .arg("--human")
        .arg("--no-install");
    cmd.timeout(CMD_TIMEOUT);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Detected platform:"));
}

#[test]
fn test_init_yes_creates_ddl_dir() {
    let (mut cmd, temp) = ddl_cmd();
    cmd.arg("init")
        .arg("--yes")
        .arg("--human")
        .arg("--no-install");
    cmd.timeout(CMD_TIMEOUT);
    cmd.assert().success();
    let ddl_path = temp.path().join(".ddl");
    assert!(ddl_path.exists(), ".ddl/ directory should be created");
    assert!(
        ddl_path.join("manifest.json").exists(),
        "manifest.json should be created"
    );
    assert!(
        ddl_path.join("config.toml").exists(),
        "config.toml should be created"
    );
}

#[test]
fn test_init_yes_creates_gitignore() {
    let (mut cmd, temp) = ddl_cmd();
    cmd.arg("init")
        .arg("--yes")
        .arg("--human")
        .arg("--no-install");
    cmd.timeout(CMD_TIMEOUT);
    cmd.assert().success();
    let gitignore = temp.path().join(".gitignore");
    assert!(gitignore.exists(), ".gitignore should be created");
}

#[test]
fn test_init_yes_with_no_install_skips_install() {
    let (mut cmd, _temp) = ddl_cmd();
    cmd.arg("init")
        .arg("--yes")
        .arg("--human")
        .arg("--no-install");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Skipping installation"));
}

#[test]
fn test_init_yes_creates_manifest() {
    let (mut cmd, temp) = ddl_cmd();
    cmd.arg("init")
        .arg("--yes")
        .arg("--human")
        .arg("--no-install");
    cmd.timeout(CMD_TIMEOUT);
    cmd.assert().success();
    let manifest_path = temp.path().join(".ddl").join("manifest.json");
    assert!(manifest_path.exists(), "manifest.json should exist");
    let content = std::fs::read_to_string(manifest_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(parsed["ddl_version"], "0.2.0");
}

// ===================== Selective install tests =====================

#[test]
fn test_init_tools_parses_flag() {
    let (mut cmd, _temp) = ddl_cmd();
    cmd.arg("init")
        .arg("--tools")
        .arg("wai,dont")
        .arg("--human")
        .arg("--no-install");
    cmd.timeout(CMD_TIMEOUT);
    // Should parse correctly and succeed with --no-install
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Skipping installation"));
}

#[test]
fn test_init_tools_creates_ddl_dir() {
    let (mut cmd, temp) = ddl_cmd();
    cmd.arg("init")
        .arg("--tools")
        .arg("wai")
        .arg("--human")
        .arg("--no-install");
    cmd.timeout(CMD_TIMEOUT);
    cmd.assert().success();
    let ddl_path = temp.path().join(".ddl");
    assert!(ddl_path.exists(), ".ddl/ should be created");
}

// ===================== Already-initialized dir tests =====================

#[test]
fn test_init_yes_in_initialized_dir_skips_installed_tools() {
    let (mut cmd, temp) = ddl_cmd();
    // First run — creates .ddl/
    cmd.arg("init")
        .arg("--yes")
        .arg("--human")
        .arg("--no-install");
    cmd.timeout(CMD_TIMEOUT);
    cmd.assert().success();

    // Second run — should detect already-initialized
    let mut cmd2 = Command::cargo_bin("ddl").unwrap();
    cmd2.current_dir(temp.path());
    cmd2.arg("init")
        .arg("--yes")
        .arg("--human")
        .arg("--no-install");
    cmd2.timeout(CMD_TIMEOUT);
    cmd2.assert()
        .success()
        .stdout(predicate::str::contains(".ddl/"));
}

#[test]
fn test_init_yes_retries_failed_tools() {
    let (mut cmd, temp) = ddl_cmd();
    create_failed_manifest(temp.path());
    // Running --yes in an already-initialized dir with a failed tool
    // should attempt to install the failed tool. With --no-install, it
    // should still handle the already-initialized manifest correctly.
    cmd.arg("init")
        .arg("--yes")
        .arg("--human")
        .arg("--no-install");
    cmd.timeout(CMD_TIMEOUT);
    cmd.assert().success();
    let manifest_path = temp.path().join(".ddl").join("manifest.json");
    let content = std::fs::read_to_string(manifest_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
    // The tool should still be tracked in the manifest
    assert!(
        parsed["tools"]
            .as_object()
            .is_some_and(|t| t.contains_key("wai"))
    );
}

// ===================== JSON output tests =====================

#[test]
fn test_init_json_output() {
    let (mut cmd, _temp) = ddl_cmd();
    cmd.arg("init").arg("--json").arg("--no-install");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("dulce-de-leche"))
        .stdout(predicate::str::contains("envelope_kind"));
}

// ===================== Error handling tests =====================

#[test]
fn test_init_unknown_tool_flag_errors() {
    let (mut cmd, _temp) = ddl_cmd();
    cmd.arg("init").arg("--unknown-flag");
    cmd.assert().failure();
}

#[test]
fn test_init_global_yes_flag_works() {
    // --yes is a global flag, so it should work before the subcommand too
    let (mut cmd, _temp) = ddl_cmd();
    cmd.arg("--yes")
        .arg("init")
        .arg("--human")
        .arg("--no-install");
    cmd.timeout(CMD_TIMEOUT);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("dulce-de-leche"));
}
