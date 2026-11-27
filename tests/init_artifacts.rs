//! Integration tests for `ddl init` artifacts (dot-ddl + version-management
//! spec): manifest structure, .gitignore append-only behavior, selective
//! tool installation, and the self-contained binary property.

mod common;

use assert_cmd::Command;
use predicates::prelude::*;
use std::time::Duration;

/// Timeout for each test.
const CMD_TIMEOUT: Duration = Duration::from_secs(120);

fn ddl_cmd(temp: &tempfile::TempDir) -> Command {
    let mut cmd = Command::cargo_bin("ddl").unwrap();
    common::clean_git_env(&mut cmd);
    cmd.current_dir(temp.path());
    cmd.timeout(CMD_TIMEOUT);
    cmd
}

fn init_yes(temp: &tempfile::TempDir, extra: &[&str]) {
    ddl_cmd(temp)
        .args(["--human", "init", "--yes", "--no-install"])
        .args(extra)
        .assert()
        .success();
}

#[test]
fn test_manifest_structure_carries_required_fields() {
    // dot-ddl/manifest-structure + version-management/manifest-structure:
    // manifest.json contains ddl_version, migration_state, and per-tool
    // entries with installed/source/status/compatible.
    let temp = tempfile::tempdir().unwrap();
    // Fake managed tools on PATH (offline): init must reconcile them into
    // the manifest so the structure has real entries to pin.
    let fake_bin = temp.path().join("fakebin");
    std::fs::create_dir_all(&fake_bin).unwrap();
    for name in ["wai", "dont"] {
        let fake = fake_bin.join(name);
        std::fs::write(&fake, "#!/usr/bin/env bash\necho 2026.11.17\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
    }
    ddl_cmd(&temp)
        .env("PATH", format!("{}:/usr/bin:/bin", fake_bin.display()))
        .args(["--human", "init", "--yes", "--no-install"])
        .assert()
        .success();
    let manifest: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(temp.path().join(".ddl/manifest.json")).unwrap(),
    )
    .unwrap();
    assert!(
        manifest.get("ddl_version").is_some(),
        "manifest must carry ddl_version, got: {manifest}"
    );
    let migration_state = manifest.get("migration_state").expect("migration_state");
    assert!(
        ["phase1", "phase2", "none"]
            .iter()
            .any(|s| migration_state == *s),
        "migration_state must be phase1|phase2|none, got {migration_state}"
    );
    let tools = manifest.get("tools").expect("tools").as_object().unwrap();
    for expected in ["wai", "dont"] {
        assert!(
            tools.contains_key(expected),
            "on-PATH tool {expected} must be reconciled into the manifest, got: {tools:?}"
        );
    }
    for (tool, entry) in tools {
        for field in ["installed", "source", "status", "compatible"] {
            assert!(
                entry.get(field).is_some(),
                "manifest tool {tool} missing field {field}"
            );
        }
        let status = entry.get("status").unwrap().as_str().unwrap();
        assert!(
            ["installed", "pending", "failed"].contains(&status),
            "tool {tool} status must be installed|pending|failed, got {status}"
        );
    }
}

#[test]
fn test_init_yes_preserves_existing_gitignore_content() {
    // dot-ddl/gitignore-skip (drift note DDL-oymq): --yes does not clobber
    // an existing .gitignore — pre-existing user content survives.
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join(".gitignore"), "# my precious stuff\ntarget/\n").unwrap();
    init_yes(&temp, &[]);
    let gitignore = std::fs::read_to_string(temp.path().join(".gitignore")).unwrap();
    assert!(
        gitignore.contains("# my precious stuff"),
        "existing .gitignore content must be preserved, got:\n{gitignore}"
    );
}

#[test]
fn test_selective_init_only_tracks_requested_tools() {
    // bootstrap/selective-install: `--tools wai` only reconciles/installs
    // the requested tool. Offline pin: a fake `wai` on PATH (no other
    // managed tools) + --no-install → the manifest tracks wai and nothing
    // else.
    let temp = tempfile::tempdir().unwrap();
    let fake_bin = temp.path().join("fakebin");
    std::fs::create_dir_all(&fake_bin).unwrap();
    let fake = fake_bin.join("wai");
    std::fs::write(&fake, "#!/usr/bin/env bash\nexit 0\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    ddl_cmd(&temp)
        .env("PATH", format!("{}:/usr/bin:/bin", fake_bin.display()))
        .args(["--human", "init", "--yes", "--no-install", "--tools", "wai"])
        .assert()
        .success();
    let manifest: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(temp.path().join(".ddl/manifest.json")).unwrap(),
    )
    .unwrap();
    let tools = manifest.get("tools").expect("tools").as_object().unwrap();
    assert!(
        tools.contains_key("wai"),
        "requested tool must be tracked, got: {tools:?}"
    );
    for unrequested in ["dont", "ah", "pretender", "testaruda"] {
        assert!(
            !tools.contains_key(unrequested),
            "unrequested tool {unrequested} must not be tracked"
        );
    }
}

#[test]
fn test_binary_runs_without_any_installation_step() {
    // bootstrap/binary-download: the ddl binary is self-contained — it runs
    // from a bare environment (no package manager, no toolchain, no config
    // files on PATH or in env) and prints its version.
    let temp = tempfile::tempdir().unwrap();
    let bin = std::path::PathBuf::from(env!("CARGO_BIN_EXE_ddl"));
    assert!(bin.exists(), "ddl binary must exist at {}", bin.display());
    let output = std::process::Command::new(&bin)
        .arg("--version")
        .env_clear()
        .env("PATH", temp.path().display().to_string())
        .output()
        .expect("self-contained binary must run with an empty environment");
    assert!(
        output.status.success(),
        "ddl --version must succeed with a bare environment"
    );
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        text.contains("ddl") && text.chars().any(|c| c.is_ascii_digit()),
        "--version must print the ddl version, got: {text:?}"
    );
}

#[test]
fn test_release_ships_the_five_platform_assets() {
    // bootstrap/release-assets: the release workflow must ship binaries for
    // all five platforms. Structural pin of .github/workflows/release.yml
    // (drift note DDL-oymq: assets are named *_darwin_* / *_windows_* /
    // *_linux_* per the workflow, not *_macos_*).
    let workflow = std::fs::read_to_string(
        std::env::current_dir()
            .unwrap()
            .ancestors()
            .find(|p| p.join(".github/workflows/release.yml").exists())
            .expect("repo root")
            .join(".github/workflows/release.yml"),
    )
    .unwrap();
    for platform in [
        "linux_amd64",
        "linux_arm64",
        "darwin_amd64",
        "darwin_arm64",
        "windows_amd64",
    ] {
        assert!(
            workflow.contains(&format!("ddl_${{VERSION}}_{platform}")),
            "release.yml must ship the {platform} asset"
        );
    }
}
