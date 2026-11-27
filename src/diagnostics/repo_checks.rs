//! Repository doctor checks: gates wiring, agent instrumentation, gitignore,
//! pins, license, fmt, bd hook chain, changelog, beads export, skills.
//!
//! Purpose: answer "is THIS repository ddl-initialized and conformant?" —
//! one check per repo-level property, each keyed off the repo's git root.
//!
//! Responsibilities:
//! - register the repo-scope checks the doctor runner runs on a repo
//!   (`report.rs` owns registration; this file owns the checks themselves)
//! - decide each check's scope from repo evidence (dirs, pins), never from a
//!   universal "every repo hosts every tool" assumption
//! - report problems as `Severity::Warning` with an actionable fix hint;
//!   `ddl doctor` never fails on a repo-scope finding
//!
//! Rationale: (DDL-ehu) agent instrumentation is verified against what each
//! tool itself publishes — a managed `AGENTS.md` block or a published
//! artifact — and only for tools the repo uses. ddl checks presence; it never
//! re-creates an artifact its writer retired (openspec's block since 1.0.0).

use super::*;
use genesis::doctor::DoctorCheck;
use genesis::suite_linter::{LintResult, Severity};
use std::path::Path;

/// What a tool publishes as its agent-instrumentation surface.
///
/// The shape encodes the ownership rule (docs/standardization.md §4.2): the
/// check verifies the tool's published surface, never that the tool
/// reproduces something it has retired.
enum Instrumentation {
    /// The tool owns a delimited region of `AGENTS.md` and rewrites it.
    Block {
        start: &'static str,
        end: &'static str,
        /// Command that writes the block; doubles as the fix hint.
        writer: &'static str,
    },
    /// The tool publishes a file; that file's presence is the signal.
    Artifact {
        path: &'static str,
        /// `(path, migration hint)` of the layout this artifact replaced.
        legacy: Option<(&'static str, &'static str)>,
        /// Command that produces or migrates the artifact.
        writer: &'static str,
    },
}

/// One instrumentation row: the owning tool, the repo evidence that scopes
/// the check to repos using that tool, and what the tool publishes.
struct InstrumentationEntry {
    /// Tool name as it appears in diagnostics.
    tool: &'static str,
    /// Repo-relative dir whose existence means "this repo uses the tool";
    /// `None` only for ddl, which always owns the repo it checks.
    dir: Option<&'static str>,
    /// `versions.ddl.toml` `[tools]` key that also scopes the check.
    pin: Option<&'static str>,
    published: Instrumentation,
}

/// The fleet's instrumentation table. Add a row when a managed tool starts
/// or stops publishing an agent surface.
const INSTRUMENTATION: &[InstrumentationEntry] = &[
    InstrumentationEntry {
        tool: "ddl",
        dir: None,
        pin: None,
        published: Instrumentation::Block {
            start: crate::DDL_AGENTS_MARKER,
            end: "<!-- DDL:END -->",
            writer: "ddl init",
        },
    },
    InstrumentationEntry {
        tool: "wai",
        dir: Some(".wai"),
        pin: Some("wai"),
        published: Instrumentation::Block {
            start: "<!-- WAI:START -->",
            end: "<!-- WAI:END -->",
            writer: "wai init",
        },
    },
    InstrumentationEntry {
        tool: "beads",
        dir: Some(".beads"),
        pin: Some("bd"),
        published: Instrumentation::Block {
            start: "BEGIN BEADS INTEGRATION",
            end: "END BEADS INTEGRATION",
            writer: "bd init --non-interactive",
        },
    },
    // openspec 1.0.0 ("The OPSX Release", PR #578) stopped writing `AGENTS.md`
    // and moved project-wide facts into `openspec/config.yaml`; `project.md`
    // is the pre-1.0 layout the artifact replaced.
    InstrumentationEntry {
        tool: "openspec",
        dir: Some("openspec"),
        pin: Some("openspec"),
        published: Instrumentation::Artifact {
            path: "openspec/config.yaml",
            legacy: Some((
                "openspec/project.md",
                "openspec update (migrates project.md into config.yaml)",
            )),
            writer: "openspec update",
        },
    },
];

impl InstrumentationEntry {
    /// This repo uses the tool when its own directory is present or the repo
    /// pins it. Neither signal is sufficient alone: genesis uses openspec
    /// without pinning it, and a repo can pin a tool it has not initialized
    /// yet (the artifact check then reports the missing artifact).
    fn is_scoped(&self, root: &Path, pins: &[String]) -> bool {
        match (self.dir, self.pin) {
            (None, None) => true,
            (dir, pin) => {
                dir.is_some_and(|d| root.join(d).exists())
                    || pin.is_some_and(|key| pins.iter().any(|p| p == key))
            }
        }
    }

    fn is_block(&self) -> bool {
        matches!(self.published, Instrumentation::Block { .. })
    }
}

/// `[tools]` keys from the repo's own `versions.ddl.toml`, if committed.
/// Unreadable or missing pins scope nothing — the dir check still applies.
fn repo_pins(root: &Path) -> Vec<String> {
    std::fs::read_to_string(root.join("versions.ddl.toml"))
        .map(|text| {
            crate::manifest::parse_fleet_pins(&text)
                .into_iter()
                .map(|(name, _)| name)
                .collect()
        })
        .unwrap_or_default()
}

/// Check the repo's lefthook gates wiring (DDL-6zn.5).
pub struct RepoGatesCheck;

impl DoctorCheck for RepoGatesCheck {
    fn name(&self) -> &'static str {
        "ddl.repo.gates"
    }
    fn description(&self) -> &'static str {
        "Check that commit/push gates are wired in this repo"
    }
    fn run(
        &self,
        repo_root: &Path,
    ) -> std::result::Result<Vec<LintResult>, Box<dyn std::error::Error>> {
        let Some(root) = find_git_root(repo_root) else {
            return Ok(vec![]);
        };
        let path = root.join("lefthook.yml");
        let content = path
            .exists()
            .then(|| std::fs::read_to_string(&path))
            .transpose()
            .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
        Ok(crate::gates::detect_lefthook_gates_problems(
            content.as_deref(),
            &crate::gates::GateCommands::for_repo(&root),
        )
        .into_iter()
        .map(|problem| LintResult::with_fix(problem, Severity::Warning, "ddl init --gates"))
        .collect())
    }
}

/// Check the repo's agent instrumentation (DDL-6zn.5, DDL-ehu).
///
/// Each scoped tool must publish what it owns: a managed `AGENTS.md` block for
/// block writers, a file for artifact writers. A layout the writer replaced is
/// reported as migration, not as a missing requirement.
pub struct RepoAgentBlocksCheck;

impl DoctorCheck for RepoAgentBlocksCheck {
    fn name(&self) -> &'static str {
        "ddl.repo.blocks"
    }
    fn description(&self) -> &'static str {
        "Check that each used tool's published agent instrumentation is present"
    }
    fn run(
        &self,
        repo_root: &Path,
    ) -> std::result::Result<Vec<LintResult>, Box<dyn std::error::Error>> {
        let Some(root) = find_git_root(repo_root) else {
            return Ok(vec![]);
        };
        let pins = repo_pins(&root);
        let scoped: Vec<&InstrumentationEntry> = INSTRUMENTATION
            .iter()
            .filter(|entry| entry.is_scoped(&root, &pins))
            .collect();

        let agents_path = root.join("AGENTS.md");
        let agents = agents_path
            .exists()
            .then(|| std::fs::read_to_string(&agents_path))
            .transpose()
            .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;

        let mut problems = Vec::new();
        let openspec_installed = crate::diagnostics::which("openspec").is_some();
        let block_tools: Vec<&str> = scoped
            .iter()
            .filter(|entry| entry.is_block())
            .map(|entry| entry.tool)
            .collect();

        match agents.as_deref() {
            // No AGENTS.md: only a scoped block-publishing tool has a claim on
            // it. Artifact publishers are checked below regardless.
            None => {
                if !block_tools.is_empty() {
                    problems.push(LintResult::with_fix(
                        format!(
                            "AGENTS.md missing — no managed blocks for {}",
                            block_tools.join(", ")
                        ),
                        Severity::Warning,
                        "ddl init",
                    ));
                }
            }
            Some(text) => {
                for entry in scoped.iter().filter(|entry| entry.is_block()) {
                    let Instrumentation::Block { start, end, writer } = entry.published else {
                        continue;
                    };
                    if !text.contains(start) || !text.contains(end) {
                        problems.push(LintResult::with_fix(
                            format!("AGENTS.md missing managed block for {}", entry.tool),
                            Severity::Warning,
                            writer,
                        ));
                    }
                }
                if let Some(finding) = retired_openspec_block_finding(text, openspec_installed) {
                    problems.push(finding);
                }
                // DDL-9kz.1: the session-close protocol block is ddl-owned.
                // Report a missing span so the ritual becomes visible to
                // harnesses that stop at the repo root, and a drifted span
                // so a mangled ritual hides nothing (the drift evidence is
                // the product). The writer is `ddl init --gates`.
                if let Some(finding) = session_protocol_finding(text) {
                    problems.push(finding);
                }
            }
        }

        for entry in scoped.iter().filter(|entry| !entry.is_block()) {
            problems.extend(artifact_findings(&root, entry, openspec_installed));
        }

        Ok(problems)
    }
}

/// DDL-9kz.1: the session-close protocol span in AGENTS.md is ddl-owned.
/// Missing markers → one warning; markers present but the body no longer
/// matches [`crate::session_protocol_block`] → one drifted warning. A
/// conforming span emits nothing.
fn session_protocol_finding(text: &str) -> Option<LintResult> {
    let start = crate::SESSION_PROTOCOL_MARKER;
    let end = "<!-- SESSION-PROTOCOL:END -->";
    let Some(begin) = text.find(start) else {
        return Some(LintResult::with_fix(
            "AGENTS.md missing the session-close protocol block — harnesses \
             that stop at the repo root never see the close ritual",
            Severity::Warning,
            "ddl init --gates",
        ));
    };
    let Some(end_idx) = text[begin..].find(end) else {
        return Some(LintResult::with_fix(
            "AGENTS.md session-close protocol block is truncated (START \
             marker without END)",
            Severity::Warning,
            "ddl init --gates",
        ));
    };
    let span = &text[begin..begin + end_idx + end.len()];
    if span != crate::session_protocol_block() {
        return Some(LintResult::with_fix(
            "AGENTS.md session-close protocol block has drifted from the \
             canonical ritual text",
            Severity::Warning,
            "ddl init --gates (rewrites the span in place)",
        ));
    }
    None
}

/// openspec 1.0.0 ("The OPSX Release", PR #578) retired this managed block: it
/// writes no `AGENTS.md`, and `openspec update` removes authentic legacy
/// copies (fingerprint-gated). ddl never writes or requires it — a repo still
/// carrying the block gets one advisory naming the supported removal, whether
/// or not it is on the artifact layout.
fn retired_openspec_block_finding(text: &str, installed: bool) -> Option<LintResult> {
    const MARKER: &str = "OPENSPEC:START";
    if !text.contains(MARKER) {
        return None;
    }
    let message = format!(
        "AGENTS.md carries the retired {MARKER} managed block — openspec 1.0.0 \
         no longer publishes it"
    );
    Some(if installed {
        LintResult::with_fix(
            message,
            Severity::Warning,
            "openspec update (removes authentic legacy blocks)",
        )
    } else {
        LintResult::new(
            format!("{message} (openspec not installed)"),
            Severity::Warning,
        )
    })
}

/// Findings for an artifact-publishing tool. Presence of the artifact passes;
/// the layout it replaced is a migration advisory; neither file on a scoped
/// repo is a missing-artifact warning. A fix hint is emitted only when the
/// tool is installed, so doctor never names a command that cannot run.
fn artifact_findings(
    root: &Path,
    entry: &InstrumentationEntry,
    installed: bool,
) -> Vec<LintResult> {
    let (path, legacy, writer) = match entry.published {
        Instrumentation::Artifact {
            path,
            legacy,
            writer,
        } => (path, legacy, writer),
        Instrumentation::Block { .. } => return vec![],
    };
    let tool = entry.tool;
    let artifact = root.join(path).exists();
    let legacy_present = legacy.is_some_and(|(legacy_path, _)| root.join(legacy_path).exists());
    let (message, fix) = match (artifact, legacy_present) {
        (true, false) => return vec![],
        (true, true) => {
            let legacy_path = legacy.map(|(legacy_path, _)| legacy_path).unwrap_or(path);
            (
                format!(
                    "{tool} hybrid layout: {path} is authoritative — migrate and \
                     delete {legacy_path}"
                ),
                writer,
            )
        }
        (false, true) => {
            let (legacy_path, hint) = legacy.unwrap_or((path, writer));
            (
                format!(
                    "{tool} legacy layout: {legacy_path} is not read by {tool} 1.x — \
                     migrate it into {path}"
                ),
                hint,
            )
        }
        (false, false) => (
            format!("{tool} is scoped but {path} is missing — the tool published no artifact"),
            writer,
        ),
    };
    let finding = if installed {
        LintResult::with_fix(message, Severity::Warning, fix)
    } else {
        LintResult::new(
            format!("{message} ({tool} not installed)"),
            Severity::Warning,
        )
    };
    vec![finding]
}

/// Check the repo's .gitignore coverage of tool data dirs (DDL-6zn.5).
pub struct RepoGitignoreCheck;

impl DoctorCheck for RepoGitignoreCheck {
    fn name(&self) -> &'static str {
        "ddl.repo.gitignore"
    }
    fn description(&self) -> &'static str {
        "Check that tool data dirs are gitignored in this repo"
    }
    fn run(
        &self,
        repo_root: &Path,
    ) -> std::result::Result<Vec<LintResult>, Box<dyn std::error::Error>> {
        let Some(root) = find_git_root(repo_root) else {
            return Ok(vec![]);
        };
        let path = root.join(".gitignore");
        let text = path
            .exists()
            .then(|| std::fs::read_to_string(&path))
            .transpose()
            .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
        let existing: Vec<String> = text
            .unwrap_or_default()
            .lines()
            .map(|l| l.trim().to_string())
            .collect();
        Ok(crate::gates::GITIGNORE_TOOL_DIR_ENTRIES
            .iter()
            .filter(|e| !existing.contains(&e.to_string()))
            .map(|e| {
                LintResult::with_fix(
                    format!("{e} not gitignored"),
                    Severity::Warning,
                    "ddl init --gates",
                )
            })
            .collect())
    }
}

/// Check the repo's version pins (DDL-6zn.5).
///
/// Only meaningful for Rust repos (a `Cargo.toml` at the repo root):
/// the standardization policy pins ecosystem CI installs in
/// `versions.ddl.toml`. Non-Rust repos emit nothing.
pub struct RepoPinsCheck;

impl DoctorCheck for RepoPinsCheck {
    fn name(&self) -> &'static str {
        "ddl.repo.pins"
    }
    fn description(&self) -> &'static str {
        "Check that ecosystem CI installs are pinned in this repo"
    }
    fn run(
        &self,
        repo_root: &Path,
    ) -> std::result::Result<Vec<LintResult>, Box<dyn std::error::Error>> {
        let Some(root) = find_git_root(repo_root) else {
            return Ok(vec![]);
        };
        if !root.join("Cargo.toml").exists() {
            return Ok(vec![]);
        }
        if !root.join("versions.ddl.toml").exists() {
            return Ok(vec![LintResult::with_fix(
                "versions.ddl.toml missing — CI installs are unpinned",
                Severity::Warning,
                "add versions.ddl.toml (docs/standardization.md §pins)",
            )]);
        }
        Ok(vec![])
    }
}

/// Check the repo's fleet-standard license posture (DDL-rod).
///
/// A LICENSE file must exist at the repo root; when the repo is Rust
/// (has `Cargo.toml`), the LICENSE text must also agree with the declared
/// `license` field — the gently audit class (declared MIT, upstream port
/// actually GPL-2.0-or-later) is mechanically detectable as a mismatch.
pub struct RepoLicenseCheck;

impl DoctorCheck for RepoLicenseCheck {
    fn name(&self) -> &'static str {
        "ddl.repo.license"
    }
    fn description(&self) -> &'static str {
        "Check that LICENSE exists and matches Cargo.toml's license field"
    }
    fn run(
        &self,
        repo_root: &Path,
    ) -> std::result::Result<Vec<LintResult>, Box<dyn std::error::Error>> {
        let Some(root) = find_git_root(repo_root) else {
            return Ok(vec![]);
        };
        let license_path = ["LICENSE", "LICENSE.md", "LICENSE.txt"]
            .iter()
            .map(|f| root.join(f))
            .find(|p| p.exists());
        let cargo_toml = root
            .join("Cargo.toml")
            .exists()
            .then(|| std::fs::read_to_string(root.join("Cargo.toml")))
            .transpose()
            .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
        let declared = cargo_toml.as_deref().and_then(declared_license);
        let mut problems = Vec::new();
        let Some(path) = license_path else {
            problems.push(LintResult::with_fix(
                "LICENSE missing at repo root",
                Severity::Warning,
                "add a LICENSE file (docs/standardization.md §fleet-compliance)",
            ));
            return Ok(problems);
        };
        let text = std::fs::read_to_string(&path)
            .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
        match declared {
            None if cargo_toml.is_some() => problems.push(LintResult::with_fix(
                "Cargo.toml declares no license field",
                Severity::Warning,
                "set package.license (docs/standardization.md §fleet-compliance)",
            )),
            Some(decl) if !license_text_matches(&decl, &text) => {
                problems.push(LintResult::with_fix(
                    format!("LICENSE text does not match declared license {decl}"),
                    Severity::Warning,
                    "reconcile LICENSE with Cargo.toml license (docs/standardization.md §fleet-compliance)",
                ));
            }
            _ => {}
        }
        Ok(problems)
    }
}

/// Read the `license = "…"` field out of a Cargo.toml, if any.
fn declared_license(cargo_toml: &str) -> Option<String> {
    cargo_toml.lines().find_map(|line| {
        let t = line.trim();
        t.strip_prefix("license =")
            .map(|rest| rest.trim().trim_matches('"').trim_matches('\'').to_string())
            .filter(|s| !s.is_empty())
    })
}

/// Best-effort match of a LICENSE file's text against a declared SPDX id.
/// Unknown ids (and `OR`/`WITH` expressions) pass — only the well-known
/// fleet licenses are mechanically verifiable.
fn license_text_matches(declared: &str, text: &str) -> bool {
    let t = text.to_ascii_lowercase();
    let d = declared.to_ascii_lowercase();
    if d.contains(" or ") || d.contains(" with ") {
        return true;
    }
    if d.starts_with("gpl-2") {
        t.contains("gnu general public license") && t.contains("version 2")
    } else if d.starts_with("gpl-3") {
        t.contains("gnu general public license") && t.contains("version 3")
    } else if d.starts_with("apache") {
        t.contains("apache license")
    } else if d.starts_with("mit") {
        t.contains("mit license") || t.contains("permission is hereby granted")
    } else {
        true
    }
}

/// Check that rustfmt is gated somewhere mechanical (DDL-rod).
///
/// The gently audit class: no `fmt`/`fmt-check` just recipes, no rustfmt in
/// CI, no fmt gate in lefthook — the repo silently opted out. The justfile
/// `fmt-check` recipe is the fleet's canonical gate surface.
pub struct RepoFmtCheck;

impl DoctorCheck for RepoFmtCheck {
    fn name(&self) -> &'static str {
        "ddl.repo.fmt"
    }
    fn description(&self) -> &'static str {
        "Check that rustfmt is gated (fmt-check recipe in justfile)"
    }
    fn run(
        &self,
        repo_root: &Path,
    ) -> std::result::Result<Vec<LintResult>, Box<dyn std::error::Error>> {
        let Some(root) = find_git_root(repo_root) else {
            return Ok(vec![]);
        };
        if !root.join("Cargo.toml").exists() {
            return Ok(vec![]);
        }
        let justfile = root
            .join("justfile")
            .exists()
            .then(|| std::fs::read_to_string(root.join("justfile")))
            .transpose()
            .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
        let gated = justfile
            .as_deref()
            .map(|t| t.lines().any(|l| l.trim().starts_with("fmt-check")))
            .unwrap_or(false);
        if gated {
            Ok(vec![])
        } else {
            Ok(vec![LintResult::with_fix(
                "rustfmt not gated — no fmt-check recipe in justfile",
                Severity::Warning,
                "add fmt/fmt-check recipes (docs/standardization.md §fleet-compliance)",
            )])
        }
    }
}

/// Check that lefthook chains the beads hooks (DDL-rod, the DDL-0tp class).
///
/// When lefthook replaces the bd shim it must chain `bd hooks run` for both
/// pre-commit and pre-push, or beads checks silently stop running.
pub struct RepoBdHookChainCheck;

impl DoctorCheck for RepoBdHookChainCheck {
    fn name(&self) -> &'static str {
        "ddl.repo.bd-hooks"
    }
    fn description(&self) -> &'static str {
        "Check that lefthook chains `bd hooks run` for pre-commit and pre-push"
    }
    fn run(
        &self,
        repo_root: &Path,
    ) -> std::result::Result<Vec<LintResult>, Box<dyn std::error::Error>> {
        let Some(root) = find_git_root(repo_root) else {
            return Ok(vec![]);
        };
        let content = root
            .join("lefthook.yml")
            .exists()
            .then(|| std::fs::read_to_string(root.join("lefthook.yml")))
            .transpose()
            .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
        let Some(text) = content else {
            // RepoGatesCheck already flags a missing lefthook.yml; the bd
            // chain check has nothing extra to say without it.
            return Ok(vec![]);
        };
        let missing: Vec<&str> = ["pre-commit", "pre-push"]
            .into_iter()
            .filter(|h| !text.contains(&format!("bd hooks run {h}")))
            .collect();
        if missing.is_empty() {
            Ok(vec![])
        } else {
            Ok(vec![LintResult::with_fix(
                format!(
                    "lefthook does not chain `bd hooks run {}`",
                    missing.join(", ")
                ),
                Severity::Warning,
                "ddl init --gates",
            )])
        }
    }
}

/// Check that a CHANGELOG.md exists at the repo root (DDL-rod).
pub struct RepoChangelogCheck;

impl DoctorCheck for RepoChangelogCheck {
    fn name(&self) -> &'static str {
        "ddl.repo.changelog"
    }
    fn description(&self) -> &'static str {
        "Check that CHANGELOG.md exists at the repo root"
    }
    fn run(
        &self,
        repo_root: &Path,
    ) -> std::result::Result<Vec<LintResult>, Box<dyn std::error::Error>> {
        let Some(root) = find_git_root(repo_root) else {
            return Ok(vec![]);
        };
        if root.join("CHANGELOG.md").exists() {
            Ok(vec![])
        } else {
            Ok(vec![LintResult::with_fix(
                "CHANGELOG.md missing at repo root",
                Severity::Warning,
                "add CHANGELOG.md (docs/standardization.md §fleet-compliance)",
            )])
        }
    }
}

/// Check the repo's beads tracked-export invariant (DDLBeads).
///
/// The 2026-10 sweep found three failure shapes across the workspace:
/// db-mode configs never flipped to no-db, exports missing entirely
/// (funkalization), and exports present but untracked/stale (gently,
/// uarup). This check flags all of them with their one-line fixes.
pub struct RepoBeadsExportCheck;

impl DoctorCheck for RepoBeadsExportCheck {
    fn name(&self) -> &'static str {
        "ddl.repo.beads-export"
    }
    fn description(&self) -> &'static str {
        "Check that .beads/issues.jsonl exists, is tracked, and is fresh"
    }
    fn run(
        &self,
        repo_root: &Path,
    ) -> std::result::Result<Vec<LintResult>, Box<dyn std::error::Error>> {
        let Some(root) = find_git_root(repo_root) else {
            return Ok(vec![]);
        };
        let beads_dir = root.join(".beads");
        if !beads_dir.is_dir() {
            return Ok(vec![]);
        }
        let config_path = beads_dir.join("config.yaml");
        let config = config_path
            .exists()
            .then(|| std::fs::read_to_string(&config_path))
            .transpose()
            .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
        let config_no_db = config
            .as_deref()
            .is_some_and(|t| t.lines().any(|l| l.trim_start().starts_with("no-db: true")));
        let jsonl = beads_dir.join("issues.jsonl");
        let jsonl_lines = jsonl
            .exists()
            .then(|| std::fs::read_to_string(&jsonl).map(|t| t.lines().count()))
            .transpose()
            .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
        let tracked = genesis::git::tracked(&root, &jsonl).ok();
        let bd_count = crate::gates::find_in_path("bd").and_then(|bd| {
            std::process::Command::new(bd)
                .arg("count")
                .current_dir(&root)
                .stdin(std::process::Stdio::null())
                .output()
                .ok()
                .filter(|o| o.status.success())
                .and_then(|o| {
                    String::from_utf8_lossy(&o.stdout)
                        .trim()
                        .parse::<u64>()
                        .ok()
                })
        });
        let input = crate::gates::BeadsExportProblemsInput {
            beads_dir_exists: true,
            config_exists: config.is_some(),
            config_no_db,
            embeddeddolt_exists: beads_dir.join("embeddeddolt").is_dir(),
            issues_jsonl_exists: jsonl.is_file(),
            issues_jsonl_tracked: tracked,
            issues_jsonl_lines: jsonl_lines,
            bd_count,
        };
        Ok(crate::gates::detect_beads_export_problems(&input)
            .into_iter()
            .map(|problem| {
                LintResult::with_fix(
                    problem,
                    Severity::Warning,
                    "bd export -o .beads/issues.jsonl && git add -f .beads/issues.jsonl",
                )
            })
            .collect())
    }
}

/// Check that incitaciones skills are installed globally.
pub struct IncitacionesSkillsCheck;

impl DoctorCheck for IncitacionesSkillsCheck {
    fn name(&self) -> &'static str {
        "ddl.incitaciones-skills"
    }
    fn description(&self) -> &'static str {
        "Check that incitaciones skills are installed globally"
    }
    fn run(
        &self,
        _repo_root: &Path,
    ) -> std::result::Result<Vec<LintResult>, Box<dyn std::error::Error>> {
        let status = crate::skills::skill_status();
        if status.global_version.is_some() {
            // DDL-6zn.4: installed-globally is a pass, not an Advisory→Warn.
            Ok(vec![])
        } else if let Some(ver) = status.local_version {
            Ok(vec![LintResult::new(
                format!(
                    "incitaciones skills only local (npm:{ver}) — run `npx incitaciones install --global` for all projects",
                    ver = ver
                ),
                Severity::Warning,
            )])
        } else {
            Ok(vec![LintResult::new(
                "incitaciones skills not installed — run `npx incitaciones install --global`",
                Severity::Warning,
            )])
        }
    }
}

/// Detect a `core.hooksPath` that points nowhere (DDL-cm1d).
///
/// The DDL-uce recurrence class: a copied or renamed checkout left a stale
/// repo-local `core.hooksPath` at a nonexistent path, so NO git hook ran in
/// that checkout — the whole gate chain (lefthook, bd validation, kaya
/// capture) was silently dead while every other sibling repo stayed healthy.
/// The recurrence risk is per-clone, which is exactly what a per-repo doctor
/// check can catch at every doctor run.
///
/// Reads the effective `core.hooksPath` (repo-local and global, as git
/// resolves it), expands `~/`, resolves a relative value against the repo
/// root (git's hook lookup behavior), and flags a value whose directory does
/// not exist at `Severity::Error` — with no hooksPath at all (the common
/// state) it emits nothing.
pub struct RepoHooksPathCheck;

impl DoctorCheck for RepoHooksPathCheck {
    fn name(&self) -> &'static str {
        "ddl.repo.hooks-path"
    }
    fn description(&self) -> &'static str {
        "Detect a core.hooksPath that points nowhere (hooks silently dead)"
    }
    fn run(
        &self,
        repo_root: &Path,
    ) -> std::result::Result<Vec<LintResult>, Box<dyn std::error::Error>> {
        let Some(root) = find_git_root(repo_root) else {
            return Ok(vec![]);
        };
        let out = std::process::Command::new("git")
            .args(["config", "--get", "core.hooksPath"])
            .current_dir(&root)
            .output()
            .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
        let value = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if value.is_empty() {
            return Ok(vec![]);
        }
        // git expands a leading `~/` in core.hooksPath against the user home.
        let expanded = if value == "~" || value.starts_with("~/") {
            match std::env::var_os("HOME") {
                Some(home) => std::path::PathBuf::from(home).join(value.trim_start_matches("~/")),
                None => std::path::PathBuf::from(&value),
            }
        } else {
            std::path::PathBuf::from(&value)
        };
        // A relative hooksPath is resolved by git against the repo root.
        let target = if expanded.is_absolute() {
            expanded
        } else {
            root.join(expanded)
        };
        if target.exists() {
            return Ok(vec![]);
        }
        Ok(vec![LintResult::with_fix(
            format!(
                "core.hooksPath '{value}' points nowhere — no git hook (gates, \
                 bd validation, record capture) runs in this checkout"
            ),
            Severity::Error,
            "fix: git config core.hooksPath <existing dir> or unset it \
             (per-clone risk — a copied .git/config carried the stale value)",
        )])
    }
}

/// Check that the repo wires the standard `just ci` gate (DDL-9kz.6.1).
///
/// The fleet standard runs every gate through `just ci` (parity between
/// local and CI, docs/standardization.md §9). A repo with no justfile, or a
/// justfile without a `ci` recipe, silently opts out of the standard gate
/// while still looking ddl-initialized. Advisory: not every repo must adopt
/// just, but the choice should be visible to the doctor reader.
pub struct RepoJustCiCheck;

impl DoctorCheck for RepoJustCiCheck {
    fn name(&self) -> &'static str {
        "ddl.repo.just-ci"
    }
    fn description(&self) -> &'static str {
        "Check that the repo's justfile carries the standard ci recipe"
    }
    fn run(
        &self,
        repo_root: &Path,
    ) -> std::result::Result<Vec<LintResult>, Box<dyn std::error::Error>> {
        let Some(root) = find_git_root(repo_root) else {
            return Ok(vec![]);
        };
        let path = ["justfile", "Justfile", ".justfile"]
            .iter()
            .map(|f| root.join(f))
            .find(|p| p.is_file());
        let Some(path) = path else {
            return Ok(vec![LintResult::with_fix(
                "no justfile — the standard 'just ci' gate is not wired",
                Severity::Warning,
                "add a justfile with a ci recipe (docs/standardization.md §9)",
            )]);
        };
        let text = std::fs::read_to_string(&path)
            .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
        let has_ci = text.lines().any(|line| {
            let line = line.trim_start();
            // recipe syntax: `ci:` or `ci args:`, optional `@` prefix,
            // and `just ci` inside a recipe line also implies it (aliases).
            if line.starts_with('#')
                || line.starts_with('@') && !line.trim_start_matches('@').starts_with("ci")
            {
                return false;
            }
            let stripped = line.trim_start_matches('@');
            let mut tokens = stripped.split_whitespace();
            let head = tokens.next().unwrap_or("");
            head == "ci:" || head == "ci"
        }) || text.lines().any(|l| l.contains("just ci"));
        if has_ci {
            return Ok(vec![]);
        }
        Ok(vec![LintResult::with_fix(
            "justfile has no ci recipe — the standard gate recipe is missing",
            Severity::Warning,
            "add a ci recipe running the standard pipeline (docs/standardization.md §9)",
        )])
    }
}

/// Check the README front-door contract (DDL-udq, docs/standardization.md §5.1).
///
/// Mandatory for every ddl-managed tool (recommended elsewhere): a cold
/// reader must be able to decide, run once, and find recurring answers from
/// the README alone. The check verifies the required section headings of the
/// v0.4 front-door contract and aggregates every miss into one finding that
/// names them all — the fix points at the canonical skeleton.
pub struct RepoReadmeFrontdoorCheck;

impl DoctorCheck for RepoReadmeFrontdoorCheck {
    fn name(&self) -> &'static str {
        "ddl.repo.readme-frontdoor"
    }
    fn description(&self) -> &'static str {
        "Check README against the front-door contract (§5.1)"
    }
    fn run(
        &self,
        repo_root: &Path,
    ) -> std::result::Result<Vec<LintResult>, Box<dyn std::error::Error>> {
        let Some(root) = find_git_root(repo_root) else {
            return Ok(vec![]);
        };
        let path = ["README.md", "readme.md", "Readme.md"]
            .iter()
            .map(|f| root.join(f))
            .find(|p| p.is_file());
        let Some(path) = path else {
            return Ok(vec![LintResult::with_fix(
                "no README.md — the front door is missing entirely (§5.1)",
                Severity::Warning,
                "add a README.md per the §5.1 front-door skeleton",
            )]);
        };
        let text = std::fs::read_to_string(&path)
            .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
        let headings: Vec<&str> = text
            .lines()
            .filter_map(|l| l.strip_prefix("## "))
            .map(|h| h.trim())
            .collect();
        let has = |needle: &str| headings.iter().any(|h| h.starts_with(needle));
        let mut missing = Vec::new();
        // 3. When to use
        if !has("When to use") {
            missing.push("## When to use");
        }
        // 4. Install (accept the common 'Installation' spelling)
        if !has("Install") {
            missing.push("## Install");
        }
        // 5. Quick start
        if !has("Quick start") {
            missing.push("## Quick start");
        }
        // 6. Common workflows
        if !has("Common workflows") {
            missing.push("## Common workflows");
        }
        // 7. Commands
        if !has("Commands") {
            missing.push("## Commands");
        }
        // 8. FAQ — inline section or a link to the faq/troubleshooting page
        if !has("FAQ")
            && !text.contains("docs/src/faq.md")
            && !text.contains("docs/src/troubleshooting.md")
        {
            missing.push("## FAQ (or a docs/src/faq.md|troubleshooting.md link)");
        }
        if missing.is_empty() {
            return Ok(vec![]);
        }
        Ok(vec![LintResult::with_fix(
            format!(
                "README front door is missing required §5.1 sections: {}",
                missing.join(", ")
            ),
            Severity::Warning,
            "fill the sections per the §5.1 canonical skeleton (docs/standardization.md §5.1)",
        )])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn git_repo() -> (tempfile::TempDir, std::path::PathBuf) {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().to_path_buf();
        std::process::Command::new("git")
            .args(["init", "-q"])
            .current_dir(&root)
            .status()
            .unwrap();
        (temp, root)
    }

    fn run_check(check: &dyn DoctorCheck, root: &std::path::Path) -> Vec<LintResult> {
        check.run(root).expect("check must not error")
    }

    // DDL-cm1d: ddl.repo.hooks-path

    #[test]
    fn hooks_path_ok_when_effective_value_resolves() {
        let (_t, root) = git_repo();
        // The check reads the EFFECTIVE config (repo-local + global). This
        // assertion therefore covers both "unset everywhere" and "a valid
        // machine-global value (e.g. ~/.git-hooks, this dev machine)" — a
        // dangling global value can only be reproduced with env mutation
        // and is out of unit-test reach; the dangling LOCAL case (the
        // DDL-uce class) is the next test.
        let out = run_check(&RepoHooksPathCheck, &root);
        assert!(out.is_empty(), "valid/unset hooksPath must pass: {out:?}");
    }

    #[test]
    fn hooks_path_rejects_nowhere_local() {
        let (_t, root) = git_repo();
        std::process::Command::new("git")
            .args(["config", "core.hooksPath", "/nonexistent/ddl-uce-case"])
            .current_dir(&root)
            .status()
            .unwrap();
        let out = run_check(&RepoHooksPathCheck, &root);
        assert!(
            !out.is_empty(),
            "hooksPath pointing at a nonexistent path must be flagged"
        );
        assert!(matches!(out[0].severity, Severity::Error));
    }

    #[test]
    fn hooks_path_ok_when_directory_exists() {
        let (_t, root) = git_repo();
        std::fs::create_dir_all(root.join("hooks-dir")).unwrap();
        std::process::Command::new("git")
            .args(["config", "core.hooksPath", "hooks-dir"])
            .current_dir(&root)
            .status()
            .unwrap();
        let out = run_check(&RepoHooksPathCheck, &root);
        assert!(out.is_empty(), "existing hooks dir must pass: {out:?}");
    }

    // DDL-9kz.6.1: ddl.repo.just-ci

    #[test]
    fn just_ci_flags_missing_justfile() {
        let (_t, root) = git_repo();
        let out = run_check(&RepoJustCiCheck, &root);
        assert_eq!(out.len(), 1, "no justfile must be flagged");
    }

    #[test]
    fn just_ci_flags_justfile_without_ci_recipe() {
        let (_t, root) = git_repo();
        std::fs::write(root.join("justfile"), "build:\n    cargo build\n").unwrap();
        let out = run_check(&RepoJustCiCheck, &root);
        assert_eq!(out.len(), 1, "justfile without a ci recipe must be flagged");
    }

    #[test]
    fn just_ci_ok_with_ci_recipe() {
        let (_t, root) = git_repo();
        std::fs::write(root.join("justfile"), "ci:\n    @just fmt-check\n").unwrap();
        let out = run_check(&RepoJustCiCheck, &root);
        assert!(out.is_empty(), "ci recipe present must pass: {out:?}");
    }

    // DDL-udq: ddl.repo.readme-frontdoor

    #[test]
    fn frontdoor_flags_missing_readme() {
        let (_t, root) = git_repo();
        let out = run_check(&RepoReadmeFrontdoorCheck, &root);
        assert_eq!(out.len(), 1, "missing README must be flagged");
    }

    #[test]
    fn frontdoor_lists_missing_sections() {
        let (_t, root) = git_repo();
        std::fs::write(
            root.join("README.md"),
            "# tool\n\n> One sentence.\n> **Why:** because.\n> **Status:** beta.\n\n## Install\n\n```sh\ncargo install tool\n```\n",
        )
        .unwrap();
        let out = run_check(&RepoReadmeFrontdoorCheck, &root);
        assert_eq!(out.len(), 1);
        let detail = &out[0].message;
        for section in ["When to use", "Quick start", "Common workflows", "FAQ"] {
            assert!(
                detail.contains(section),
                "missing '{section}' must be listed, got: {detail}"
            );
        }
        assert!(
            !detail.contains("Install"),
            "present section must not be listed"
        );
    }

    #[test]
    fn frontdoor_ok_with_all_sections() {
        let (_t, root) = git_repo();
        std::fs::write(
            root.join("README.md"),
            "# tool\n\n> One sentence.\n> **Why:** because.\n> **Status:** beta.\n\n## When to use\n\n- use it\n\n## Install\n\n```sh\ncargo install tool\n```\n\n## Quick start\n\n```sh\ntool run\n# → ok\n```\n\n## Common workflows\n\n1. job — `cmd` → result\n\n## Commands\n\n| c | d |\n|---|---|\n\n## FAQ\n\n- q?\n\n## License\n\nMIT\n",
        )
        .unwrap();
        let out = run_check(&RepoReadmeFrontdoorCheck, &root);
        assert!(out.is_empty(), "complete front door must pass: {out:?}");
    }
}
