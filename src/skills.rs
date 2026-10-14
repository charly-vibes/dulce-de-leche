//! incitaciones skill detection and installation.
//!
//! incitaciones distributes prompts as skills via npm. Skills are installed
//! into an `skills/` directory (global: `~/.agents/skills/`, local:
//! `.agents/skills/`) where each skill's `SKILL.md` frontmatter carries the
//! provenance marker `installed-from: incitaciones` and the installed
//! npm version. Detection scans for that marker — no subprocess needed.
//!
//! Rationale (DDL-1z1, SEC-02): the npm child is a bootstrap subprocess, so
//! it runs under the shared child runtime budget — a hung `npm install` is
//! killed (with its process group) within the configured deadline and the
//! exhausted budget is named in the failure.

use std::path::PathBuf;

use crate::error::{DdlError, Result};

/// The provenance marker written by the incitaciones installer into each
/// skill's SKILL.md frontmatter.
const INSTALLED_FROM_MARKER: &str = "installed-from: incitaciones";

// Rationale (DDL-uu8, SEC-04): global skill installation mutates
// ~/.agents/skills/, which every project on the machine reads — so that scope
// requires explicit policy authorization (DDL_ALLOW_GLOBAL_SKILL=1) and its
// child runs under the restricted environment policy from crate::installer.
// Local installs stay project-scoped.

/// The npm package name.
pub const NPM_PACKAGE: &str = "incitaciones";

/// Where incitaciones skills were found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillStatus {
    /// npm version of globally installed skills (e.g. "0.8.0"), if any.
    pub global_version: Option<String>,
    /// npm version of locally (project) installed skills, if any.
    pub local_version: Option<String>,
}

impl SkillStatus {
    /// Skills are considered available when installed globally (ddl's
    /// preferred scope) — a local-only install still prompts for global.
    pub fn has_global(&self) -> bool {
        self.global_version.is_some()
    }
}

/// The global skills directory: `~/.agents/skills/`.
pub fn global_skills_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".agents").join("skills"))
}

/// The local skills directory: `.agents/skills/`, resolved by walking up
/// from CWD (mirrors `find_ddl_dir`) so running ddl from a subdirectory
/// still finds a repo-root install.
pub fn local_skills_dir() -> Option<PathBuf> {
    let mut cwd = std::env::current_dir().ok()?;
    loop {
        let candidate = cwd.join(".agents").join("skills");
        if candidate.is_dir() {
            return Some(candidate);
        }
        if !cwd.pop() {
            return None;
        }
    }
}

/// Scan a skills directory for incitaciones-installed skills and return the
/// npm version recorded in their frontmatter (first match wins).
fn detect_in_dir(dir: &std::path::Path) -> Option<String> {
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let skill_md = entry.path().join("SKILL.md");
        let Ok(contents) = std::fs::read_to_string(&skill_md) else {
            continue;
        };
        // Frontmatter is at the top of the file — check the first 20 lines.
        let frontmatter: String = contents.lines().take(20).collect::<Vec<_>>().join("\n");
        if frontmatter.contains(INSTALLED_FROM_MARKER)
            && let Some(version) = extract_version(&frontmatter)
        {
            return Some(version);
        }
    }
    None
}

/// Extract the npm version from `installed-version: "npm:0.8.0"`.
fn extract_version(frontmatter: &str) -> Option<String> {
    let line = frontmatter
        .lines()
        .find(|l| l.trim_start().starts_with("installed-version:"))?;
    let idx = line.find("npm:")? + "npm:".len();
    let version = line[idx..].trim().trim_matches('"');
    if version.is_empty() {
        None
    } else {
        Some(version.to_string())
    }
}

/// Detect where incitaciones skills are installed (global and/or local).
pub fn skill_status() -> SkillStatus {
    SkillStatus {
        global_version: global_skills_dir().as_deref().and_then(detect_in_dir),
        local_version: local_skills_dir().and_then(|d| detect_in_dir(&d)),
    }
}

/// Install incitaciones skills via the incitaciones CLI.
///
/// Uses the globally-installed `incitaciones` binary when available,
/// falling back to `npx --yes incitaciones` otherwise.
pub fn install_skills(global: bool, verbose: bool) -> Result<()> {
    let scope_arg = if global { "--global" } else { "--local" };

    // DDL-uu8 (SEC-04): the global scope writes to ~/.agents/skills — visible
    // to every project on the machine — so it requires explicit policy
    // authorization (DDL_ALLOW_GLOBAL_SKILL=1). Local scope writes stay inside
    // this project and need no extra grant. Fail-closed: no silent fallback.
    let scope = if global {
        crate::installer::ExecutionScope::GlobalSkill
    } else {
        crate::installer::ExecutionScope::ToolInit
    };
    crate::installer::authorize_scope(scope)?;

    let (cmd, base_args): (&str, Vec<&str>) = if which_incitaciones().is_some() {
        ("incitaciones", vec!["install", scope_arg])
    } else {
        ("npx", vec!["--yes", NPM_PACKAGE, "install", scope_arg])
    };

    if verbose {
        eprintln!("  · running: {cmd} {}", base_args.join(" "));
    }

    // DDL-uu8 policy first (env allowlist + scope authorization), then
    // DDL-1z1's runtime budget bounds the spawn; both diagnostics propagate
    // unchanged so failures name the exhausted budget or rejected policy.
    let mut command = crate::installer::npm_command(cmd);
    crate::installer::apply_child_policy(&mut command, scope, &[]).map_err(|e| {
        DdlError::InstallFailed(format!("skill install rejected by child policy: {e}"))
    })?;
    let status = crate::installer::run_child_bounded(
        command
            .args(&base_args)
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit()),
        crate::installer::child_deadline(),
    )?;

    if status.success() {
        Ok(())
    } else {
        Err(DdlError::InstallFailed(format!(
            "{cmd} {scope_arg} exited with code {}",
            status.code().unwrap_or(-1)
        )))
    }
}

/// Check whether the incitaciones CLI is on PATH.
///
/// On Windows the npm-installed binary is an `incitaciones.cmd` shim —
/// probe the shim too so detection works there.
fn which_incitaciones() -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|paths| {
        for dir in std::env::split_paths(&paths) {
            #[cfg(windows)]
            let candidates = [dir.join("incitaciones.exe"), dir.join("incitaciones.cmd")];
            #[cfg(not(windows))]
            let candidates = [dir.join("incitaciones")];
            for full in candidates {
                if full.is_file() {
                    return Some(full);
                }
            }
        }
        None
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_version() {
        let fm = "name: commit\ninstalled-from: incitaciones\ninstalled-version: \"npm:0.8.0\"";
        assert_eq!(extract_version(fm), Some("0.8.0".to_string()));
    }

    #[test]
    fn test_extract_version_missing() {
        let fm = "name: commit\ninstalled-from: incitaciones";
        assert_eq!(extract_version(fm), None);
    }

    /// DDL-uu8: the global-skill scope is rejected without explicit
    /// authorization; the local (tool-init) scope needs no grant.
    #[test]
    fn global_skill_scope_requires_explicit_authorization() {
        let lookup = |var: &str| {
            if var == crate::installer::ALLOW_GLOBAL_SKILL_ENV {
                None // simulate the authorization var not being set
            } else {
                std::env::var(var).ok()
            }
        };
        assert!(
            crate::installer::scope_authorize_with(
                crate::installer::ExecutionScope::GlobalSkill,
                lookup
            )
            .is_err()
        );
        let authorized = |var: &str| {
            if var == crate::installer::ALLOW_GLOBAL_SKILL_ENV {
                Some("1".to_string())
            } else {
                std::env::var(var).ok()
            }
        };
        assert!(
            crate::installer::scope_authorize_with(
                crate::installer::ExecutionScope::GlobalSkill,
                authorized
            )
            .is_ok()
        );
        assert!(
            crate::installer::scope_authorize_with(
                crate::installer::ExecutionScope::ToolInit,
                authorized
            )
            .is_ok()
        );
    }

    #[test]
    fn test_marker_detection() {
        let fm = "---\nname: commit\ninstalled-from: incitaciones\ninstalled-version: \"npm:0.8.0\"\n---";
        assert!(fm.contains(INSTALLED_FROM_MARKER));
        let other = "---\nname: commit\ninstalled-from: something-else\n---";
        assert!(!other.contains(INSTALLED_FROM_MARKER));
    }
}
