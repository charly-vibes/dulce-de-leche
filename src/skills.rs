//! incitaciones skill detection and installation.
//!
//! incitaciones distributes prompts as skills via npm. Skills are installed
//! into an `skills/` directory (global: `~/.agents/skills/`, local:
//! `.agents/skills/`) where each skill's `SKILL.md` frontmatter carries the
//! provenance marker `installed-from: incitaciones` and the installed
//! npm version. Detection scans for that marker — no subprocess needed.

use std::path::PathBuf;

use crate::error::{DdlError, Result};

/// The provenance marker written by the incitaciones installer into each
/// skill's SKILL.md frontmatter.
const INSTALLED_FROM_MARKER: &str = "installed-from: incitaciones";

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

/// The local skills directory: `.agents/skills/` (relative to CWD).
pub fn local_skills_dir() -> PathBuf {
    PathBuf::from(".agents").join("skills")
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
        global_version: global_skills_dir()
            .as_deref()
            .and_then(detect_in_dir),
        local_version: detect_in_dir(&local_skills_dir()),
    }
}

/// Install incitaciones skills via the incitaciones CLI.
///
/// Uses the globally-installed `incitaciones` binary when available,
/// falling back to `npx --yes incitaciones` otherwise.
pub fn install_skills(global: bool, verbose: bool) -> Result<()> {
    let scope_arg = if global { "--global" } else { "--local" };

    let (cmd, base_args): (&str, Vec<&str>) = if which_incitaciones().is_some() {
        ("incitaciones", vec!["install", scope_arg])
    } else {
        ("npx", vec!["--yes", NPM_PACKAGE, "install", scope_arg])
    };

    if verbose {
        eprintln!("  · running: {cmd} {}", base_args.join(" "));
    }

    let status = crate::installer::npm_command(cmd)
        .args(&base_args)
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status()
        .map_err(|e| DdlError::InstallFailed(format!("Failed to run {cmd}: {e}")))?;

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
            let candidates = [
                dir.join("incitaciones.exe"),
                dir.join("incitaciones.cmd"),
            ];
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

    #[test]
    fn test_marker_detection() {
        let fm = "---\nname: commit\ninstalled-from: incitaciones\ninstalled-version: \"npm:0.8.0\"\n---";
        assert!(fm.contains(INSTALLED_FROM_MARKER));
        let other = "---\nname: commit\ninstalled-from: something-else\n---";
        assert!(!other.contains(INSTALLED_FROM_MARKER));
    }
}
