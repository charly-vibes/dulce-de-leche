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
//! - §4  pinned ecosystem installs (`versions.ddl.toml` committed, and each
//!   §4-matrix ✅ tool wired into `ci.yml` at the committed pin)
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
    "genesis",
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
        } else if *name == "incitaciones" {
            "npm-publish.yml"
        } else {
            // lib crates publish to crates.io on tag (no binary matrix)
            "publish.yml"
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

// ---------- §1 canonical release naming ----------

// §1: archives are named {tool}_{VERSION}_{os}_{arch} (.tar.gz, windows .zip).
// os/arch use the short amd64/arm64 forms, not target triples. The tool name
// may differ from the repo name (espectacular ships `ah`, whisper ships
// `turu`, dulce-de-leche ships `ddl`). The four .tar.gz variants may be
// spelled out or produced by a `for plat in …` loop over exactly those
// platforms (testaruda/vampiro idiom); the windows .zip must be literal.
#[test]
fn s1_release_uses_canonical_naming() {
    const TAR_VARIANTS: &[&str] = &[
        "linux_amd64.tar.gz",
        "linux_arm64.tar.gz",
        "darwin_amd64.tar.gz",
        "darwin_arm64.tar.gz",
    ];
    for name in RUST_REPOS {
        let Some(dir) = repo_dir(name) else { continue };
        let rel = read(&dir, ".github/workflows/release.yml")
            .unwrap_or_else(|| panic!("{name}: missing release.yml"));
        let tool = match *name {
            "espectacular" => "ah",
            "whisper" => "turu",
            "dulce-de-leche" => "ddl",
            other => other,
        };
        assert!(
            rel.contains(&format!("{tool}_${{VERSION}}_windows_amd64.zip")),
            "{name}: release.yml must publish '{tool}_${{VERSION}}_windows_amd64.zip' (§1 canonical naming)"
        );
        let tar_ok = TAR_VARIANTS
            .iter()
            .all(|v| rel.contains(&format!("{tool}_${{VERSION}}_{v}")))
            || (rel.contains("for plat in linux_amd64 linux_arm64 darwin_amd64 darwin_arm64")
                && rel.contains(&format!("{tool}_${{VERSION}}_${{plat}}.tar.gz")));
        assert!(
            tar_ok,
            "{name}: release.yml must publish '{tool}_${{VERSION}}_{{linux,darwin}}_{{amd64,arm64}}.tar.gz' (§1 canonical naming)"
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
fn s2_book_src_is_docs_src() {
    for name in ALL_REPOS {
        let Some(dir) = repo_dir(name) else { continue };
        let book = read(&dir, "book.toml")
            .unwrap_or_else(|| panic!("{name}: book.toml must live at repo root (§2)"));
        assert!(
            book.contains(r#"src = "docs/src""#),
            "{name}: book.toml must set src = \"docs/src\" (§2 book pages live in docs/src/)"
        );
        assert!(
            dir.join("docs/src/SUMMARY.md").is_file(),
            "{name}: missing docs/src/SUMMARY.md (§2)"
        );
    }
}

#[test]
fn s2_specs_deployed_in_docs() {
    for name in ALL_REPOS {
        let Some(dir) = repo_dir(name) else { continue };
        let specs_dir = dir.join("openspec/specs");
        let has_specs = fs::read_dir(&specs_dir)
            .map(|entries| {
                entries
                    .filter_map(|e| e.ok())
                    .any(|e| e.path().is_dir() && e.path().join("spec.md").is_file())
            })
            .unwrap_or(false);
        if !has_specs {
            continue; // no openspec specs → nothing to deploy
        }
        let docs_yml = read(&dir, ".github/workflows/docs.yml")
            .unwrap_or_else(|| panic!("{name}: missing .github/workflows/docs.yml (§1)"));
        let deploys_specs =
            docs_yml.contains("docs/src/specs") || docs_yml.contains("build_docs.py");
        assert!(
            deploys_specs,
            "{name}: docs.yml must copy openspec specs into the book source (docs/src/specs) or assemble them via build_docs.py (§2)"
        );
        let summary = read(&dir, "docs/src/SUMMARY.md")
            .unwrap_or_else(|| panic!("{name}: missing docs/src/SUMMARY.md (§2)"));
        assert!(
            summary.contains("./specs/")
                || summary.contains("(specs/")
                || summary.contains("spec.md"),
            "{name}: SUMMARY.md must link the deployed spec pages so they render in the book (§2)"
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
        assert!(
            dir.join("llm.txt").is_file(),
            "{name}: missing llm.txt at repo root (§3 layout: narrative summary alongside llms.txt)"
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

/// §4 dogfooding matrix: required (✅) columns per consumer row.
/// `◻ opt` and `—` (self) are excluded. `genesis` is absent: lib-crate
/// carve-out (§4) — its CI installs no in-org family tools.
const DOGFOOD_MATRIX: &[(&str, &[&str])] = &[
    ("wai", &["pretender", "bd", "openspec", "dulce-de-leche"]),
    (
        "testaruda",
        &["pretender", "wai", "bd", "openspec", "dulce-de-leche"],
    ),
    (
        "dont",
        &["pretender", "wai", "bd", "openspec", "dulce-de-leche"],
    ),
    ("pretender", &["wai", "bd", "openspec", "dulce-de-leche"]),
    (
        "vampiro",
        &[
            "pretender",
            "wai",
            "bd",
            "openspec",
            "dont",
            "dulce-de-leche",
        ],
    ),
    (
        "specodelic",
        &["pretender", "wai", "bd", "openspec", "dulce-de-leche"],
    ),
    ("incitaciones", &["pretender", "wai", "bd", "dulce-de-leche"]),
    (
        "espectacular",
        &["pretender", "wai", "bd", "openspec", "dulce-de-leche"],
    ),
    ("whisper", &["pretender", "wai", "bd", "dulce-de-leche"]),
    (
        "dulce-de-leche",
        &["pretender", "wai", "bd", "openspec", "dont"],
    ),
];

/// The install reference each tool's pin must produce in `ci.yml`.
/// One canonical install form per tool keeps fleet CI uniform and the
/// assertion mechanical:
/// - crates.io tools: `cargo install <pkg>@<pin>` (package name, not bin name)
/// - npm tools: `<pkg>@<pin>`
/// - bd: GitHub release asset `beads_<pin>_<target>.tar.gz` (no versioned
///   homebrew-core formula — release download is the pinned mechanism)
fn install_marker(tool: &str, pin: &str) -> String {
    match tool {
        "pretender" | "espectacular" | "specodelic" | "dulce-de-leche" => {
            format!("{tool}@{pin}")
        }
        "wai" => format!("wai-cli@{pin}"),
        "dont" => format!("dont-cli@{pin}"),
        "openspec" => format!("@fission-ai/openspec@{pin}"),
        "bd" => format!("beads_{pin}_"),
        other => panic!("unknown dogfood tool {other}"),
    }
}

/// Minimal `key = "value"` parser for `versions.ddl.toml` `[tools]` entries
/// (comment- and whitespace-tolerant; deliberately avoids a toml dependency).
fn parse_pins(src: &str) -> Vec<(String, String)> {
    src.lines()
        .filter_map(|line| {
            let line = line.split('#').next()?.trim();
            let (key, value) = line.split_once('=')?;
            Some((key.trim().to_string(), value.trim().trim_matches('"').to_string()))
        })
        .collect()
}

#[test]
fn s4_ci_installs_dogfood_matrix() {
    for (name, required) in DOGFOOD_MATRIX {
        let Some(dir) = repo_dir(name) else { continue };
        let pins_src = read(&dir, "versions.ddl.toml")
            .unwrap_or_else(|| panic!("{name}: missing versions.ddl.toml (§4)"));
        let ci = read(&dir, ".github/workflows/ci.yml")
            .unwrap_or_else(|| panic!("{name}: missing ci.yml (§4)"));
        let pins = parse_pins(&pins_src);
        for tool in *required {
            let Some((_, pin)) = pins.iter().find(|(k, _)| k == tool) else {
                panic!(
                    "{name}: §4 matrix requires {tool} but versions.ddl.toml has no pin for it"
                );
            };
            let marker = install_marker(tool, pin);
            assert!(
                ci.contains(&marker),
                "{name}: ci.yml does not install {tool} at the committed pin \
                 (expected `{marker}`) — §4: installs must be wired per the dogfooding matrix"
            );
        }
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
