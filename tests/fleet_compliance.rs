//! Fleet-standard compliance checks (DDL-rod).
//!
//! The gently compliance audit surfaced four mechanically detectable gaps:
//! no LICENSE (or a LICENSE that contradicts `Cargo.toml`'s license field),
//! no rustfmt gate, lefthook not chaining `bd hooks run`, and no CHANGELOG.
//! These tests pin the `ddl.repo.*` doctor checks that detect each gap.

use assert_cmd::Command;
use serde_json::Value;
use std::time::Duration;

const CMD_TIMEOUT: Duration = Duration::from_secs(10);

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
