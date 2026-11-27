//! Fleet-standard compliance checks (DDL-rod).
//!
//! The gently compliance audit surfaced four mechanically detectable gaps:
//! no LICENSE (or a LICENSE that contradicts `Cargo.toml`'s license field),
//! no rustfmt gate, lefthook not chaining `bd hooks run`, and no CHANGELOG.
//! These tests pin the `ddl.repo.*` doctor checks that detect each gap.

use assert_cmd::Command;
use serde_json::Value;
use std::time::Duration;

mod common;

const CMD_TIMEOUT: Duration = Duration::from_secs(60);

/// Run `ddl doctor --json` inside `dir` and return the diagnostics array.
fn doctor_diagnostics(dir: &std::path::Path) -> Vec<Value> {
    let mut cmd = Command::cargo_bin("ddl").unwrap();
    cmd.current_dir(dir);
    cmd.args(["doctor", "--json"]).timeout(CMD_TIMEOUT);
    let out = cmd.assert().success().get_output().stdout.clone();
    let envelope: Value = serde_json::from_slice(&out).expect("valid JSON envelope");
    envelope["data"]["diagnostics"]
        .as_array()
        .cloned()
        .expect("diagnostics array")
}

/// Find diagnostics for a check id at a given level (or any level if None).
fn items<'a>(diags: &'a [Value], check: &str, level: Option<&str>) -> Vec<&'a Value> {
    diags
        .iter()
        .filter(|d| d["check"] == check)
        .filter(|d| level.is_none_or(|l| d["level"] == l))
        .collect()
}

/// A git repo root (the doctor repo checks key off a `.git` entry).
fn temp_repo() -> (tempfile::TempDir, std::path::PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().to_path_buf();
    std::fs::create_dir_all(root.join(".git")).unwrap();
    (temp, root)
}

const MIT_LICENSE: &str = "MIT License\n\nCopyright (c) 2026 charly vibes\n";
const APACHE_LICENSE: &str = "Apache License\nVersion 2.0, January 2004\n";
const GPL2_LICENSE: &str =
    "GNU GENERAL PUBLIC LICENSE\nVersion 2, June 1991\n\nEveryone is permitted to copy...\n";

const CONFORMING_JUSTFILE: &str = "fmt:\n    cargo fmt\n\nfmt-check:\n    cargo fmt -- --check\n";
const CONFORMING_LEFTHOOK: &str = "pre-commit:\n  commands:\n    fmt-check:\n      run: just fmt-check\n    bd:\n      run: bd hooks run pre-commit\n\npre-push:\n  commands:\n    bd:\n      run: bd hooks run pre-push\n";

/// A Rust repo with every audited gap: no LICENSE, no fmt recipe,
/// lefthook that does not chain `bd hooks run`, no CHANGELOG.
fn gap_repo() -> (tempfile::TempDir, std::path::PathBuf) {
    let (temp, root) = temp_repo();
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"x\"\nlicense = \"MIT\"\n",
    )
    .unwrap();
    std::fs::write(root.join("justfile"), "test:\n    cargo test\n").unwrap();
    std::fs::write(
        root.join("lefthook.yml"),
        "pre-commit:\n  commands:\n    lint:\n      run: cargo clippy\n",
    )
    .unwrap();
    (temp, root)
}

#[test]
fn all_four_gaps_are_flagged() {
    let (_t, root) = gap_repo();
    let diags = doctor_diagnostics(&root);

    let license = items(&diags, "ddl.repo.license", Some("warn"));
    assert!(
        !license.is_empty(),
        "missing LICENSE must be flagged; diagnostics: {diags:?}"
    );
    assert!(
        license[0]["detail"]
            .as_str()
            .unwrap()
            .to_lowercase()
            .contains("license"),
        "detail should mention the license gap"
    );

    assert!(
        !items(&diags, "ddl.repo.fmt", Some("warn")).is_empty(),
        "missing rustfmt gate (no fmt-check recipe) must be flagged"
    );
    assert!(
        !items(&diags, "ddl.repo.bd-hooks", Some("warn")).is_empty(),
        "lefthook not chaining `bd hooks run` must be flagged"
    );
    assert!(
        !items(&diags, "ddl.repo.changelog", Some("warn")).is_empty(),
        "missing CHANGELOG.md must be flagged"
    );
}

#[test]
fn license_mismatch_with_cargo_toml_is_flagged() {
    let (temp, root) = temp_repo();
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"x\"\nlicense = \"MIT\"\n",
    )
    .unwrap();
    // Apache text under a MIT declaration — the gently-shaped mismatch.
    std::fs::write(root.join("LICENSE"), APACHE_LICENSE).unwrap();
    std::fs::write(root.join("CHANGELOG.md"), "# Changelog\n").unwrap();
    std::fs::write(root.join("justfile"), CONFORMING_JUSTFILE).unwrap();
    std::fs::write(root.join("lefthook.yml"), CONFORMING_LEFTHOOK).unwrap();

    let diags = doctor_diagnostics(temp.path());
    let license = items(&diags, "ddl.repo.license", Some("warn"));
    assert!(
        !license.is_empty(),
        "LICENSE/Cargo.toml license mismatch must be flagged"
    );
    assert!(
        license[0]["detail"].as_str().unwrap().contains("MIT"),
        "detail should name the declared license"
    );
}

#[test]
fn cargo_toml_without_license_field_is_flagged() {
    let (temp, root) = temp_repo();
    std::fs::write(root.join("Cargo.toml"), "[package]\nname = \"x\"\n").unwrap();
    std::fs::write(root.join("LICENSE"), MIT_LICENSE).unwrap();
    std::fs::write(root.join("CHANGELOG.md"), "# Changelog\n").unwrap();
    std::fs::write(root.join("justfile"), CONFORMING_JUSTFILE).unwrap();
    std::fs::write(root.join("lefthook.yml"), CONFORMING_LEFTHOOK).unwrap();

    let diags = doctor_diagnostics(temp.path());
    assert!(
        !items(&diags, "ddl.repo.license", Some("warn")).is_empty(),
        "Cargo.toml without a license field must be flagged"
    );
}

#[test]
fn conforming_repo_passes_all_four_checks() {
    let (temp, root) = temp_repo();
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"x\"\nlicense = \"MIT\"\n",
    )
    .unwrap();
    std::fs::write(root.join("LICENSE"), MIT_LICENSE).unwrap();
    std::fs::write(root.join("CHANGELOG.md"), "# Changelog\n").unwrap();
    std::fs::write(root.join("justfile"), CONFORMING_JUSTFILE).unwrap();
    std::fs::write(root.join("lefthook.yml"), CONFORMING_LEFTHOOK).unwrap();

    let diags = doctor_diagnostics(temp.path());
    for check in [
        "ddl.repo.license",
        "ddl.repo.fmt",
        "ddl.repo.bd-hooks",
        "ddl.repo.changelog",
    ] {
        assert!(
            items(&diags, check, Some("warn")).is_empty(),
            "conforming repo must not warn on {check}; diagnostics: {diags:?}"
        );
        assert!(
            !items(&diags, check, Some("pass")).is_empty(),
            "check {check} should register a pass item on a conforming repo"
        );
    }
}

#[test]
fn gpl2_license_matches_gpl2_or_later_declaration() {
    let (temp, root) = temp_repo();
    // The gently failure mode: declared MIT, upstream port must be GPL.
    // Here the corrected shape — GPL-2.0-or-later declaration with GPL-2 text.
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"x\"\nlicense = \"GPL-2.0-or-later\"\n",
    )
    .unwrap();
    std::fs::write(root.join("LICENSE"), GPL2_LICENSE).unwrap();
    std::fs::write(root.join("CHANGELOG.md"), "# Changelog\n").unwrap();
    std::fs::write(root.join("justfile"), CONFORMING_JUSTFILE).unwrap();
    std::fs::write(root.join("lefthook.yml"), CONFORMING_LEFTHOOK).unwrap();

    let diags = doctor_diagnostics(temp.path());
    assert!(
        items(&diags, "ddl.repo.license", Some("warn")).is_empty(),
        "GPL-2 text under a GPL-2.0-or-later declaration must pass; diagnostics: {diags:?}"
    );
}

#[test]
fn non_rust_repo_still_checks_license_and_changelog_presence() {
    let (temp, root) = temp_repo();
    // No Cargo.toml — license *matching* is inapplicable but presence checks
    // stay live for the whole fleet.
    std::fs::write(root.join("CHANGELOG.md"), "# Changelog\n").unwrap();
    std::fs::write(root.join("justfile"), "fmt-check:\n    echo fmt\n").unwrap();
    std::fs::write(root.join("lefthook.yml"), CONFORMING_LEFTHOOK).unwrap();

    let diags = doctor_diagnostics(temp.path());
    assert!(
        !items(&diags, "ddl.repo.license", Some("warn")).is_empty(),
        "LICENSE presence is checked even without Cargo.toml"
    );
    assert!(items(&diags, "ddl.repo.changelog", Some("warn")).is_empty());
}

#[test]
fn checks_silent_outside_git_repo() {
    let temp = tempfile::tempdir().unwrap();
    let diags = doctor_diagnostics(temp.path());
    for check in [
        "ddl.repo.license",
        "ddl.repo.fmt",
        "ddl.repo.bd-hooks",
        "ddl.repo.changelog",
    ] {
        assert!(
            items(&diags, check, None).is_empty(),
            "no {check} items outside a git repo"
        );
    }
}

// ── DDL-9kz.1: session-close protocol block ───────────────────────────

use dulce_de_leche::gates::apply_gates_wiring_with_sad;

/// Gates wiring writes the harness-agnostic session-close ritual into
/// AGENTS.md as a second managed block (next to the DDL block).
#[test]
fn session_protocol_block_written_by_gates_wiring() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    std::fs::create_dir_all(root.join(".git")).unwrap();

    // Real init order: repo setup writes the DDL block first, gates wiring
    // appends the session block after it.
    dulce_de_leche::installer::ensure_agents_block(root).unwrap();
    apply_gates_wiring_with_sad(root, None).unwrap();

    let agents = std::fs::read_to_string(root.join("AGENTS.md")).unwrap();
    for needle in [
        "<!-- SESSION-PROTOCOL:START -->",
        "<!-- SESSION-PROTOCOL:END -->",
        "bd export -o .beads/issues.jsonl && git add -f .beads/issues.jsonl && git commit",
        "just ci",
        "git push",
        "wai close",
    ] {
        assert!(agents.contains(needle), "block missing '{needle}'");
    }
    // Harness-agnostic: no harness name tokens in the block span.
    let span = agents
        .split("<!-- SESSION-PROTOCOL:START -->")
        .nth(1)
        .and_then(|s| s.split("<!-- SESSION-PROTOCOL:END -->").next())
        .unwrap_or("");
    let tokens: Vec<String> = span
        .split(|c: char| !c.is_ascii_alphanumeric())
        .map(str::to_lowercase)
        .collect();
    assert!(
        !tokens.contains(&"omp".to_string()) && !tokens.contains(&"codex".to_string()),
        "block must not name harnesses: {span}"
    );
    // Next to the DDL block: the session block sits after the DDL block
    // that repo setup wrote, not clobbering it.
    let ddl_end = agents.find("<!-- DDL:END -->").unwrap();
    let session_start = agents.find("<!-- SESSION-PROTOCOL:START -->").unwrap();
    assert!(
        session_start > ddl_end,
        "session block must sit after the DDL block"
    );
}

/// Idempotent (second wiring run is byte-identical) and self-owned
/// (a drifted span is rewritten by the next wiring run).
#[test]
fn session_protocol_block_idempotent_and_drift_rewritten() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    std::fs::create_dir_all(root.join(".git")).unwrap();

    apply_gates_wiring_with_sad(root, None).unwrap();
    let first = std::fs::read_to_string(root.join("AGENTS.md")).unwrap();

    // Drift: an agent (or a careless edit) dropped the bd export line.
    let drifted = first.replace("bd export -o .beads/issues.jsonl", "");
    assert_ne!(drifted, first);
    std::fs::write(root.join("AGENTS.md"), &drifted).unwrap();

    apply_gates_wiring_with_sad(root, None).unwrap();
    let second = std::fs::read_to_string(root.join("AGENTS.md")).unwrap();

    assert_eq!(
        first, second,
        "wiring must be idempotent and must rewrite a drifted span"
    );
}

/// The doctor flags a missing session-protocol span (the AGENTS.md-missing
/// case is covered by the scoped-block warning) and a drifted span.
#[test]
fn doctor_flags_missing_and_drifted_session_protocol_block() {
    // Missing: an AGENTS.md without the block.
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    std::fs::create_dir_all(root.join(".git")).unwrap();
    std::fs::write(root.join("AGENTS.md"), "# notes\n").unwrap();
    let diags = doctor_diagnostics(root);
    let missing = items(&diags, "ddl.repo.blocks", Some("warn"));
    assert!(
        missing.iter().any(|i| i["detail"]
            .as_str()
            .is_some_and(|d| d.contains("session-close protocol"))),
        "missing span must be flagged: {missing:?}"
    );

    // Drifted: markers present, ritual text mangled.
    let drifted_root = tempfile::tempdir().unwrap();
    let dr = drifted_root.path();
    std::fs::create_dir_all(dr.join(".git")).unwrap();
    std::fs::write(
        dr.join("AGENTS.md"),
        "<!-- SESSION-PROTOCOL:START -->\nritual text elided by a careless edit\n<!-- SESSION-PROTOCOL:END -->\n",
    )
    .unwrap();
    let diags = doctor_diagnostics(dr);
    let drifted = items(&diags, "ddl.repo.blocks", Some("warn"));
    assert!(
        drifted.iter().any(|i| i["detail"]
            .as_str()
            .is_some_and(|d| d.contains("drifted"))),
        "drifted span must be flagged: {drifted:?}"
    );
}

// ── DDL-9kz.2: beads export enforcement + gitignore negation ──────────

/// A real git repo (the negation test runs real `git add`).
fn real_git_repo() -> (tempfile::TempDir, std::path::PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().to_path_buf();
    let mut cmd = std::process::Command::new("git");
    for var in [
        "GIT_DIR", "GIT_WORK_TREE", "GIT_INDEX_FILE", "GIT_OBJECT_DIRECTORY",
        "GIT_COMMON_DIR", "GIT_CONFIG", "GIT_CONFIG_GLOBAL", "GIT_CONFIG_SYSTEM",
        "GIT_NAMESPACE",
    ] {
        cmd.env_remove(var);
    }
    let out = cmd.args(["init", "-q"]).current_dir(&root).output().unwrap();
    assert!(out.status.success());
    (temp, root)
}

/// After gates wiring, the repo-local .gitignore carries the `.beads/`
/// negation so a plain `git add .beads/issues.jsonl` succeeds even when a
/// global gitignore excludes `.beads/` — no `-f` needed (DDL-9kz.2).
#[test]
fn beads_export_gitignore_negation_makes_plain_add_work() {
    let (_temp, root) = real_git_repo();

    // A global config whose excludesFile ignores `.beads/` — the fleet-wide
    // state the negation must override. Passed per-command: no process-env
    // mutation (edition 2024).
    let global_dir = tempfile::tempdir().unwrap();
    let excludes = global_dir.path().join("global-gitignore");
    std::fs::write(&excludes, ".beads/\n").unwrap();
    let global_cfg = global_dir.path().join("global-config");
    std::fs::write(
        &global_cfg,
        format!("[core]\n\texcludesFile = {}\n", excludes.display()),
    )
    .unwrap();

    // First commit so the repo has a HEAD (git add works either way, but a
    // committed index makes the assert meaningful).
    std::fs::write(root.join("README.md"), "# x\n").unwrap();
    let _ = std::process::Command::new("git")
        .args(["add", "README.md"])
        .current_dir(&root)
        .env("GIT_CONFIG_GLOBAL", &global_cfg)
        .output()
        .unwrap();
    let _ = std::process::Command::new("git")
        .args(["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-q", "-m", "init"])
        .current_dir(&root)
        .env("GIT_CONFIG_GLOBAL", &global_cfg)
        .output()
        .unwrap();

    apply_gates_wiring_with_sad(&root, None).unwrap();
    let gitignore = std::fs::read_to_string(root.join(".gitignore")).unwrap();
    assert!(
        gitignore.contains("!.beads/"),
        "wiring must append the beads negation: {gitignore}"
    );

    // Export something and add it plainly — must NOT be swallowed.
    std::fs::create_dir_all(root.join(".beads")).unwrap();
    std::fs::write(root.join(".beads/issues.jsonl"), "{\"id\":\"t1\"}\n").unwrap();
    let out = std::process::Command::new("git")
        .args(["add", ".beads/issues.jsonl"])
        .current_dir(&root)
        .env("GIT_CONFIG_GLOBAL", &global_cfg)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "plain add must succeed after negation: {:?}",
        String::from_utf8_lossy(&out.stderr)
    );
    // And the embedded DB dir stays ignored.
    std::fs::create_dir_all(root.join(".beads/embeddeddolt")).unwrap();
    let check = std::process::Command::new("git")
        .args(["check-ignore", "-q", ".beads/embeddeddolt/x"])
        .current_dir(&root)
        .env("GIT_CONFIG_GLOBAL", &global_cfg)
        .output()
        .unwrap();
    assert!(
        check.status.success(),
        ".beads/embeddeddolt must stay ignored"
    );
}

/// `ddl doctor --strict` exits non-zero when any repo diagnostic is at
/// warn/fail level (the enforcement hook for the beads-export contract and
/// the fleet sweep).
#[test]
fn doctor_strict_exits_nonzero_on_findings() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    std::fs::create_dir_all(root.join(".git")).unwrap();
    std::fs::write(root.join("AGENTS.md"), "# notes\n").unwrap();

    let mut cmd = Command::cargo_bin("ddl").unwrap();
    common::clean_git_env(&mut cmd);
    cmd.current_dir(root);
    cmd.args(["doctor", "--json", "--strict"]);
    let out = cmd.assert().get_output().stdout.clone();
    let envelope: Value = serde_json::from_slice(&out).expect("valid envelope");
    let warns = envelope["data"]["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|d| d["level"] == "warn" || d["level"] == "fail");
    assert!(warns, "gap repo has warn findings");
    // The envelope printed (report consumed), then the process failed.
    assert_eq!(envelope["ok"], serde_json::json!(true));
    let non_strict = {
        let mut cmd = Command::cargo_bin("ddl").unwrap();
        common::clean_git_env(&mut cmd);
        cmd.current_dir(root);
        cmd.args(["doctor", "--json"]);
        cmd.assert().success().get_output().stdout.clone()
    };
    let _: Value = serde_json::from_slice(&non_strict).unwrap();
    let mut strict_cmd = Command::cargo_bin("ddl").unwrap();
    common::clean_git_env(&mut strict_cmd);
    strict_cmd.current_dir(root);
    strict_cmd.args(["doctor", "--json", "--strict"]);
    strict_cmd.assert().failure();
}

/// Strict is quiet on a fully wired repo.
#[test]
fn doctor_strict_exits_zero_when_wired() {
    let mut cmd = Command::cargo_bin("ddl").unwrap();
    common::clean_git_env(&mut cmd);
    // wired_git_repo lives in doctor_command.rs; rebuild the minimal wired
    // state here via the same fixture pieces.
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    std::fs::create_dir_all(root.join(".git")).unwrap();
    std::fs::create_dir_all(root.join(".ddl")).unwrap();
    std::fs::write(
        root.join("AGENTS.md"),
        format!(
            "<!-- WAI:START -->\n<!-- WAI:END -->\n<!-- BEGIN BEADS INTEGRATION -->\n<!-- END BEADS INTEGRATION -->\n<!-- DDL:START -->\n<!-- DDL:END -->\n{}",
            dulce_de_leche::session_protocol_block()
        ),
    )
    .unwrap();
    std::fs::write(
        root.join("justfile"),
        "ci:\n    @just fmt-check lint test\n\nfmt:\n    cargo fmt\n\nfmt-check:\n    cargo fmt -- --check\n",
    )
    .unwrap();
    std::fs::write(root.join("LICENSE"), "MIT License\n\nCopyright (c) 2026\n").unwrap();
    std::fs::write(root.join("CHANGELOG.md"), "# Changelog\n").unwrap();
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"x\"\nlicense = \"MIT\"\n",
    )
    .unwrap();
    std::fs::write(
        root.join("versions.ddl.toml"),
        "[tools]\nwai = \"latest\"\n",
    )
    .unwrap();
    std::fs::write(root.join("README.md"), "# x\n\n> one\n\n## When to use\n\n## Install\n\n## Quick start\n\n## Common workflows\n\n## Commands\n\n## FAQ\n").unwrap();
    std::fs::write(root.join(".gitignore"), ".testaruda/\n.pretender/\n").unwrap();
    let lefthook = format!(
        "pre-commit:\n  commands:\n{}\n    bd:\n      run: bd hooks run pre-commit\n\npre-push:\n  commands:\n{}\n    bd:\n      run: bd hooks run pre-push\n",
        dulce_de_leche::gates::PRE_COMMIT_BLOCK,
        dulce_de_leche::gates::PRE_PUSH_BLOCK
    );
    std::fs::write(root.join("lefthook.yml"), lefthook).unwrap();
    cmd.current_dir(root);
    cmd.args(["doctor", "--json", "--strict"]);
    cmd.assert().success();
}

// ── DDL-9kz.4: generated gate blocks chain bd hooks run ───────────────

/// PREMISE CORRECTED: the ticket records that the generated blocks do NOT
/// chain `bd hooks run`; the canonical template has done so since the
/// 9kz.3 adoption sweep (`CANONICAL_LEFTHOOK_TEMPLATE` renders both `bd
/// hooks run pre-commit` and `bd hooks run pre-push`). This regression
/// test pins the agreement between the generator and the
/// `ddl.repo.bd-hooks` fix hint.
#[test]
fn gates_wiring_generated_blocks_chain_bd_hooks() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    std::fs::create_dir_all(root.join(".git")).unwrap();

    apply_gates_wiring_with_sad(root, None).unwrap();
    let lefthook = std::fs::read_to_string(root.join("lefthook.yml")).unwrap();
    assert!(
        lefthook.contains("bd hooks run pre-commit")
            && lefthook.contains("bd hooks run pre-push"),
        "generated lefthook.yml must chain bd hooks for both hooks: {lefthook}"
    );

    // And the doctor agrees: no ddl.repo.bd-hooks finding.
    let diags = doctor_diagnostics(root);
    let findings = items(&diags, "ddl.repo.bd-hooks", Some("warn"));
    assert!(
        findings.is_empty(),
        "a ddl-init-wired repo must satisfy ddl.repo.bd-hooks: {findings:?}"
    );
}
