//! Ecosystem standard conformance drift test.
//!
//! Checks every sibling repo **present locally** (and ddl itself) against the
//! mechanically-checkable invariants of `docs/standardization.md`:
//!
//! - §1  workflow files: `ci.yml` (runs `just ci`), `docs.yml`, release slot
//!   (`release.yml` with the 5-target binary matrix; `incitaciones` maps
//!   its `npm-publish.yml` to the release slot)
//! - §2  root `book.toml` with required fields; `llms.txt` at root
//! - §3  charly theme: `default-theme = "coal"`, `additional-css` includes
//!   `theme/charly.css`, and the file exists
//! - §4  pinned ecosystem installs (`versions.ddl.toml` committed)
//! - §5  README motivation + status block (`> **Why:**` / `> **Status:**`)
//!
//! Fleet conformance is a local/ddl-side check: repos not checked out are
//! skipped, and each repo's own CI validates only itself.

use std::fs;
use std::path::{Path, PathBuf};

const RUST_REPOS: &[&str] = &[
    "wai",
    "testaruda",
    "dont",
    "pretender",
    "vampiro",
    "specodelic",
    "espectacular",
    "whisper",
    "dulce-de-leche",
];
const ALL_REPOS: &[&str] = &[
    "wai",
    "testaruda",
    "dont",
    "pretender",
    "vampiro",
    "specodelic",
    "espectacular",
    "whisper",
    "dulce-de-leche",
    "incitaciones",
];

const RELEASE_TARGETS: &[&str] = &[
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
    "x86_64-apple-darwin",
    "aarch64-apple-darwin",
    "x86_64-pc-windows-msvc",
];

fn repo_dir(name: &str) -> Option<PathBuf> {
    // ddl itself lives in the workspace root; siblings sit next to it.
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let dir = root.join("..").join(name);
    dir.is_dir().then_some(dir)
}

fn read(dir: &Path, rel: &str) -> Option<String> {
    fs::read_to_string(dir.join(rel)).ok()
}

// ---------- §1 CI ----------

#[test]
fn s1_ci_workflow_exists_and_runs_just_ci() {
    for name in ALL_REPOS {
        let Some(dir) = repo_dir(name) else { continue };
        let ci = read(&dir, ".github/workflows/ci.yml")
            .unwrap_or_else(|| panic!("{name}: missing .github/workflows/ci.yml"));
        assert!(ci.contains("just ci"), "{name}: ci.yml must run `just ci`");
    }
}

#[test]
fn s1_docs_workflow_exists() {
    for name in ALL_REPOS {
        let Some(dir) = repo_dir(name) else { continue };
        let has_docs = read(&dir, ".github/workflows/docs.yml").is_some();
        assert!(
            has_docs,
            "{name}: missing .github/workflows/docs.yml (§1: pages.yml is not the docs slot)"
        );
    }
}

#[test]
fn s1_release_slot_exists() {
    for name in ALL_REPOS {
        let Some(dir) = repo_dir(name) else { continue };
        let slot = if RUST_REPOS.contains(name) {
            "release.yml"
        } else {
            "npm-publish.yml"
        };
        assert!(
            read(&dir, &format!(".github/workflows/{slot}")).is_some(),
            "{name}: missing release slot .github/workflows/{slot}"
        );
    }
}

#[test]
fn s1_release_publishes_all_five_targets() {
    for name in RUST_REPOS {
        let Some(dir) = repo_dir(name) else { continue };
        let rel = read(&dir, ".github/workflows/release.yml")
            .unwrap_or_else(|| panic!("{name}: missing release.yml"));
        for target in RELEASE_TARGETS {
            assert!(
                rel.contains(target),
                "{name}: release.yml does not build {target} (§1 binary release matrix)"
            );
        }
        assert!(
            rel.contains("checksums.txt"),
            "{name}: release.yml does not publish checksums.txt"
        );
    }
}

// ---------- §2 documentation structure ----------

#[test]
fn s2_root_book_toml_with_required_fields() {
    for name in ALL_REPOS {
        let Some(dir) = repo_dir(name) else { continue };
        let book = read(&dir, "book.toml")
            .unwrap_or_else(|| panic!("{name}: book.toml must live at repo root (§2)"));
        assert!(book.contains("[book]"), "{name}: book.toml missing [book]");
        assert!(
            book.contains("authors"),
            "{name}: book.toml must set authors"
        );
        assert!(
            book.contains("description"),
            "{name}: book.toml must set description"
        );
    }
}

#[test]
fn s2_llms_txt_at_root() {
    for name in ALL_REPOS {
        let Some(dir) = repo_dir(name) else { continue };
        assert!(
            dir.join("llms.txt").is_file(),
            "{name}: missing llms.txt at repo root (§2)"
        );
    }
}

// ---------- §3 charly theme ----------

#[test]
fn s3_book_uses_charly_theme() {
    for name in ALL_REPOS {
        let Some(dir) = repo_dir(name) else { continue };
        let book = read(&dir, "book.toml")
            .unwrap_or_else(|| panic!("{name}: book.toml must live at repo root (§3)"));
        assert!(
            book.contains(r#"default-theme = "coal""#),
            "{name}: book.toml must set default-theme = \"coal\" (§3)"
        );
        assert!(
            book.contains("theme/charly.css"),
            "{name}: book.toml must add theme/charly.css via additional-css (§3)"
        );
        assert!(
            dir.join("theme/charly.css").is_file(),
            "{name}: missing vendored theme/charly.css (§3)"
        );
    }
}

// ---------- §4 dogfooding pins ----------

#[test]
fn s4_pinned_versions_file_committed() {
    for name in ALL_REPOS {
        let Some(dir) = repo_dir(name) else { continue };
        assert!(
            dir.join("versions.ddl.toml").is_file(),
            "{name}: missing versions.ddl.toml (§4: ecosystem CI installs must be pinned)"
        );
    }
}

// ---------- §5 motivation & status ----------

#[test]
fn s5_readme_has_why_and_status_block() {
    for name in ALL_REPOS {
        let Some(dir) = repo_dir(name) else { continue };
        let readme = read(&dir, "README.md").unwrap_or_else(|| panic!("{name}: missing README.md"));
        assert!(
            readme.contains("> **Why:**"),
            "{name}: README missing `> **Why:**` block (§5)"
        );
        assert!(
            readme.contains("> **Status:**"),
            "{name}: README missing `> **Status:**` block (§5)"
        );
        assert!(
            readme.contains("docs/src/status.md"),
            "{name}: README status must link docs/src/status.md (§5)"
        );
    }
}

#[test]
fn s5_status_page_exists() {
    for name in ALL_REPOS {
        let Some(dir) = repo_dir(name) else { continue };
        assert!(
            dir.join("docs/src/status.md").is_file(),
            "{name}: missing docs/src/status.md (§5)"
        );
    }
}
