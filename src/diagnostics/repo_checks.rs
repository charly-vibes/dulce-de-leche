//! Repository doctor checks: gates wiring, managed blocks, gitignore, pins, license, fmt, bd hook chain, changelog, beads export, skills.

use super::*;
use genesis::doctor::DoctorCheck;
use genesis::suite_linter::{LintResult, Severity};
use std::path::Path;

/// Managed blocks expected in the repo's AGENTS.md (the "this repo is
/// agent-instrumented" definition): ddl's own block (DDL-fw7 — `ddl init`
/// writes it), wai, openspec, beads.
const AGENT_BLOCKS: &[(&str, &str)] = &[
    (crate::DDL_AGENTS_MARKER, "ddl"),
    ("<!-- WAI:START -->", "wai"),
    ("OPENSPEC:START", "openspec"),
    ("BEGIN BEADS INTEGRATION", "beads"),
];

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
        Ok(
            crate::gates::detect_lefthook_gates_problems(content.as_deref())
                .into_iter()
                .map(|problem| LintResult::with_fix(problem, Severity::Warning, "ddl init --gates"))
                .collect(),
        )
    }
}

/// Check the repo's AGENTS.md managed blocks (DDL-6zn.5).
pub struct RepoAgentBlocksCheck;

impl DoctorCheck for RepoAgentBlocksCheck {
    fn name(&self) -> &'static str {
        "ddl.repo.blocks"
    }
    fn description(&self) -> &'static str {
        "Check that AGENTS.md carries the ecosystem managed blocks"
    }
    fn run(
        &self,
        repo_root: &Path,
    ) -> std::result::Result<Vec<LintResult>, Box<dyn std::error::Error>> {
        let Some(root) = find_git_root(repo_root) else {
            return Ok(vec![]);
        };
        let path = root.join("AGENTS.md");
        if !path.exists() {
            return Ok(vec![LintResult::with_fix(
                "AGENTS.md missing — tool managed blocks absent",
                Severity::Warning,
                "ddl init",
            )]);
        }
        let text = std::fs::read_to_string(&path)
            .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
        Ok(AGENT_BLOCKS
            .iter()
            .filter(|(marker, _)| !text.contains(marker))
            .map(|(_, label)| {
                LintResult::with_fix(
                    format!("AGENTS.md missing managed block for {label}"),
                    Severity::Warning,
                    "ddl init",
                )
            })
            .collect())
    }
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
        let tracked = std::process::Command::new("git")
            .args(["ls-files", "--error-unmatch", ".beads/issues.jsonl"])
            .current_dir(&root)
            .stdin(std::process::Stdio::null())
            .output()
            .ok()
            .map(|o| o.status.success());
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
