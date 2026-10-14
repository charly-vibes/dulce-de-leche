//! Integration tests for `ddl init` as the canonical per-repo setup entry
//! point (DDL-fw7).
//!
//! A fresh `ddl init -y` in a stock charly repo must leave `ddl doctor`
//! clean for the repo checks it owns:
//! - writes ddl's own managed block into AGENTS.md (genesis::aix format,
//!   idempotent on re-run)
//! - reconciles pre-existing tool installs into `.ddl/manifest.json`
//!   (doctor used to flag "not tracked in manifest" with no fix)
//! - scaffolds `versions.ddl.toml` from the fleet pin when absent
//!   (doctor hints it, never creates it)

use assert_cmd::Command;
use std::path::Path;
use std::time::Duration;

const CMD_TIMEOUT: Duration = Duration::from_secs(30);

const DDL_MARKER: &str = "<!-- DDL:START -->";

/// A git repo root (the doctor repo checks key off a `.git` entry).
fn git_init(path: &Path) {
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

/// A stock charly repo: git, AGENTS.md with the other ecosystem blocks but
/// no DDL block, Rust pins-relevant (Cargo.toml), no versions.ddl.toml.
fn stock_repo() -> tempfile::TempDir {
    let temp = tempfile::tempdir().unwrap();
    git_init(temp.path());
    std::fs::write(
        temp.path().join("AGENTS.md"),
        "<!-- WAI:START -->\n<!-- WAI:END -->\n<!-- OPENSPEC:START -->\n<!-- OPENSPEC:END -->\n<!-- BEGIN BEADS INTEGRATION -->\n<!-- END BEADS INTEGRATION -->\n",
    )
    .unwrap();
    std::fs::write(
        temp.path().join("Cargo.toml"),
        "[package]\nname = \"x\"\nlicense = \"MIT\"\n",
    )
    .unwrap();
    temp
}

/// Prepend stub executables (name → body) on a leading PATH dir so init's
/// tool probes run the stubs instead of real fleet tools. Keeps the tests
/// hermetic: no real tool init cascades, no network.
fn stub_tools_on_path(cmd: &mut Command, temp: &tempfile::TempDir, tools: &[(&str, &str)]) {
    let bin_dir = temp.path().join("stub-bin");
    std::fs::create_dir_all(&bin_dir).unwrap();
    for (name, body) in tools {
        let stub = bin_dir.join(name);
        std::fs::write(&stub, body).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
    }
    let inherited = std::env::var_os("PATH").unwrap_or_default();
    let mut paths = vec![bin_dir];
    paths.extend(std::env::split_paths(&inherited));
    cmd.env("PATH", std::env::join_paths(paths).unwrap());
}

/// A stub that answers `--version` with `<name> <version>` and exits 0 for
/// everything else (tool init cascades).
fn version_stub(version: &str) -> String {
    format!("#!/bin/sh\ncase \"$1\" in --version) echo \"{version}\" ;; *) exit 0 ;; esac\n")
}

fn ddl_init(temp: &tempfile::TempDir) -> assert_cmd::assert::Assert {
    ddl_init_args(temp, &["init", "--yes", "--human", "--no-install"])
}

fn ddl_init_args(temp: &tempfile::TempDir, args: &[&str]) -> assert_cmd::assert::Assert {
    let mut cmd = Command::cargo_bin("ddl").unwrap();
    cmd.current_dir(temp.path());
    cmd.timeout(CMD_TIMEOUT);
    stub_tools_on_path(
        &mut cmd,
        temp,
        &[
            ("incitaciones", "#!/bin/sh\nexit 0\n"),
            ("wai", &version_stub("wai 2026.9.28")),
        ],
    );
    cmd.args(args);
    cmd.assert()
}

// ===================== DDL managed block in AGENTS.md =====================

#[test]
fn test_init_writes_ddl_agents_block() {
    let temp = stock_repo();
    ddl_init(&temp);
    let agents = std::fs::read_to_string(temp.path().join("AGENTS.md")).unwrap();
    assert!(
        agents.contains(DDL_MARKER),
        "init must write the DDL managed block into AGENTS.md:\n{agents}"
    );
    assert!(
        agents.contains("ddl init"),
        "DDL block must describe the init entry point"
    );
    // genesis::aix format: START marker precedes END marker
    let start = agents.find(DDL_MARKER).unwrap();
    assert!(agents.contains("<!-- DDL:END -->"));
    let end = agents.find("<!-- DDL:END -->").unwrap();
    assert!(start < end);
    // Pre-existing ecosystem blocks survive
    assert!(agents.contains("<!-- WAI:START -->"));
}

#[test]
fn test_init_agents_block_is_idempotent() {
    let temp = stock_repo();
    ddl_init(&temp);
    ddl_init(&temp);
    let agents = std::fs::read_to_string(temp.path().join("AGENTS.md")).unwrap();
    assert_eq!(
        agents.matches(DDL_MARKER).count(),
        1,
        "re-running init must not duplicate the DDL block:\n{agents}"
    );
}

// ===================== versions.ddl.toml scaffolding =====================

#[test]
fn test_init_scaffolds_versions_ddl_toml() {
    let temp = stock_repo();
    assert!(!temp.path().join("versions.ddl.toml").exists());
    ddl_init(&temp);
    let pins = std::fs::read_to_string(temp.path().join("versions.ddl.toml")).unwrap();
    assert!(
        pins.contains("[tools]"),
        "init must scaffold versions.ddl.toml from the fleet pin:\n{pins}"
    );
}

#[test]
fn test_init_never_overwrites_existing_versions_ddl_toml() {
    let temp = stock_repo();
    std::fs::write(
        temp.path().join("versions.ddl.toml"),
        "[tools]\nwai = \"0.0.1-test\"\n",
    )
    .unwrap();
    ddl_init(&temp);
    let pins = std::fs::read_to_string(temp.path().join("versions.ddl.toml")).unwrap();
    assert!(
        pins.contains("0.0.1-test"),
        "existing pins file must be left untouched:\n{pins}"
    );
}

// ===================== manifest reconciliation =====================

#[test]
fn test_init_reconciles_preexisting_install_into_manifest() {
    let temp = stock_repo();
    // `wai` is already installed (stub) but .ddl/manifest.json has no entry —
    // doctor used to flag "wai not tracked in manifest" with no fix.
    ddl_init(&temp);
    let manifest = std::fs::read_to_string(temp.path().join(".ddl/manifest.json")).unwrap();
    let json: serde_json::Value = serde_json::from_str(&manifest).unwrap();
    let wai = &json["tools"]["wai"];
    assert!(
        wai.is_object(),
        "init must reconcile pre-existing installs into .ddl/manifest.json:\n{manifest}"
    );
    assert_eq!(wai["status"], "installed");
    assert_eq!(wai["installed"], "2026.9.28");
}

// ===================== doctor clean after init (the MUST) =====================

/// The ticket MUST: `ddl init -y` on a fresh stock charly repo leaves
/// `ddl doctor` clean for the repo.* checks (0 fail, 0 warn). A stock charly
/// repo carries the fleet-standard files (LICENSE, CHANGELOG, fmt-check
/// justfile); everything else — managed blocks, pins, manifest inventory,
/// gates wiring — is init's job.
#[test]
fn test_init_gates_leaves_doctor_repo_checks_clean() {
    let temp = stock_repo();
    // Fleet-standard files a stock charly repo already commits.
    std::fs::write(
        temp.path().join("LICENSE"),
        "MIT License\n\nPermission is hereby granted...\n",
    )
    .unwrap();
    std::fs::write(temp.path().join("CHANGELOG.md"), "# Changelog\n").unwrap();
    std::fs::write(
        temp.path().join("justfile"),
        "fmt:\n    cargo fmt\n\nfmt-check:\n    cargo fmt -- --check\n",
    )
    .unwrap();

    ddl_init_args(
        &temp,
        &["init", "--yes", "--human", "--no-install", "--gates"],
    );

    let mut cmd = Command::cargo_bin("ddl").unwrap();
    cmd.current_dir(temp.path());
    cmd.timeout(CMD_TIMEOUT);
    cmd.args(["doctor", "--json"]);
    let out = cmd.assert().success().get_output().stdout.clone();
    let envelope: serde_json::Value = serde_json::from_slice(&out).unwrap();
    let diags = envelope["data"]["diagnostics"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let repo_findings: Vec<String> = diags
        .iter()
        .filter(|d| {
            d["check"].as_str().unwrap_or("").starts_with("ddl.repo.") && d["level"] != "pass"
        })
        .map(|d| format!("{} {}: {}", d["check"], d["level"], d["detail"]))
        .collect();
    assert!(
        repo_findings.is_empty(),
        "ddl init -y --gates must leave ddl doctor clean for ddl.repo.* checks:\n{repo_findings:?}"
    );
}
