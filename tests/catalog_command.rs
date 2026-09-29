//! Integration tests for the ddl catalog command (DDL-91a).
//!
//! The catalog answers "what tools exist, what does each do, how to invoke"
//! from the ONE registry (MANAGED_TOOLS) — no second source of truth.

use assert_cmd::Command;
use dulce_de_leche::platform::MANAGED_TOOLS;
use predicates::prelude::*;
use std::time::Duration;

/// Timeout for each test.
const CMD_TIMEOUT: Duration = Duration::from_secs(10);

#[test]
fn test_catalog_help_shows_description() {
    let mut cmd = Command::cargo_bin("ddl").unwrap();
    cmd.arg("catalog").arg("--help");
    cmd.timeout(CMD_TIMEOUT);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("ddl catalog"));
}

#[test]
fn test_catalog_human_lists_every_registry_tool() {
    let mut cmd = Command::cargo_bin("ddl").unwrap();
    cmd.arg("catalog").arg("--human");
    cmd.timeout(CMD_TIMEOUT);
    let output = cmd.assert().success().get_output().stdout.clone();
    let stdout = String::from_utf8(output).unwrap();
    for tool in MANAGED_TOOLS {
        assert!(
            stdout.contains(tool.name),
            "catalog output missing tool `{}`",
            tool.name
        );
        assert!(
            stdout.contains(tool.description),
            "catalog output missing purpose for `{}`",
            tool.name
        );
    }
}

#[test]
fn test_catalog_json_envelope() {
    let temp = tempfile::tempdir().unwrap();
    let mut cmd = Command::cargo_bin("ddl").unwrap();
    cmd.current_dir(temp.path());
    cmd.arg("catalog").arg("--json");
    cmd.timeout(CMD_TIMEOUT);
    let output = cmd.assert().success().get_output().stdout.clone();
    let stdout = String::from_utf8(output).unwrap();

    // Genesis envelope shape.
    assert!(
        stdout.contains("envelope_kind"),
        "catalog --json must emit a genesis envelope"
    );

    // Parse and verify structure: every registry tool present with
    // binary + one-line purpose.
    let value: serde_json::Value = serde_json::from_str(&stdout)
        .expect("catalog --json stdout must be a single JSON envelope");
    let tools = value["data"]["tools"]
        .as_array()
        .expect("envelope data.tools must be an array");
    assert_eq!(
        tools.len(),
        MANAGED_TOOLS.len(),
        "catalog must cover every MANAGED_TOOLS entry"
    );
    for tool in MANAGED_TOOLS {
        let entry = tools
            .iter()
            .find(|t| t["name"] == tool.name)
            .unwrap_or_else(|| panic!("catalog JSON missing tool `{}`", tool.name));
        assert_eq!(
            entry["binary"], tool.name,
            "binary field must name the invoked command"
        );
        assert_eq!(
            entry["description"], tool.description,
            "description field must carry the one-line purpose"
        );
    }
}
