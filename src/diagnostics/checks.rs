//! Core doctor checks: platform, prerequisites, .ddl/ dir, per-tool health.

use super::*;
use genesis::doctor::DoctorCheck;
use genesis::suite_linter::{LintResult, Severity};
use std::path::Path;
use std::process::Command;

pub struct PlatformCheck;

impl DoctorCheck for PlatformCheck {
    fn name(&self) -> &'static str {
        "ddl.platform"
    }
    fn description(&self) -> &'static str {
        "Check that the current platform is supported"
    }
    fn run(
        &self,
        _repo_root: &Path,
    ) -> std::result::Result<Vec<LintResult>, Box<dyn std::error::Error>> {
        match crate::platform::Platform::detect() {
            // DDL-6zn.4: informational lines are not issues — empty results
            // yield a genesis pass entry. Advisory→Warn mapping would
            // otherwise poison the summary (pass: 0 warn: 37).
            Some(_) => Ok(vec![]),
            None => Ok(vec![LintResult::new(
                "Could not detect platform",
                Severity::Error,
            )]),
        }
    }
}

/// Check that prerequisites are available on PATH.
pub struct PrerequisitesCheck;

impl DoctorCheck for PrerequisitesCheck {
    fn name(&self) -> &'static str {
        "ddl.prerequisites"
    }
    fn description(&self) -> &'static str {
        "Check that required tools are on PATH"
    }
    fn run(
        &self,
        _repo_root: &Path,
    ) -> std::result::Result<Vec<LintResult>, Box<dyn std::error::Error>> {
        let mut results = Vec::new();
        for prereq in &["curl", "git", "cargo"] {
            if which(prereq).is_none() {
                // DDL-6zn.4: missing prerequisites were never reported —
                // only their presence was (as Advisory→Warn noise).
                let severity = if *prereq == "cargo" {
                    Severity::Warning // cargo is only the install fallback
                } else {
                    Severity::Error
                };
                results.push(LintResult::new(
                    format!("{prereq} not found on PATH"),
                    severity,
                ));
            }
        }
        Ok(results)
    }
}

/// Check the .ddl/ directory state.
pub struct DdlDirCheck {
    ddl_dir: Option<DdlDir>,
}

impl DdlDirCheck {
    pub fn new(ddl_dir: Option<DdlDir>) -> Self {
        Self { ddl_dir }
    }
}

impl DoctorCheck for DdlDirCheck {
    fn name(&self) -> &'static str {
        "ddl.directory"
    }
    fn description(&self) -> &'static str {
        "Check .ddl/ directory structure"
    }
    fn run(
        &self,
        _repo_root: &Path,
    ) -> std::result::Result<Vec<LintResult>, Box<dyn std::error::Error>> {
        let mut results = Vec::new();
        match &self.ddl_dir {
            Some(d) => {
                // DDL-6zn.4: presence lines are informational, not warnings —
                // only actual problems produce results.
                if !d.manifest_path().exists() {
                    results.push(LintResult::new("manifest.json not found", Severity::Error));
                }
                if !d.config_path().exists() {
                    results.push(LintResult::new(
                        "config.toml not found (created on next init)",
                        Severity::Warning,
                    ));
                }
                let broken = d.detect_broken_symlinks();
                if !broken.is_empty() {
                    for symlink in &broken {
                        results.push(LintResult::with_fix(
                            format!("broken symlink: {}", symlink.display()),
                            Severity::Error,
                            "ddl migrate --undo",
                        ));
                    }
                }
            }
            None => {
                results.push(LintResult::with_fix(
                    "No .ddl/ directory found",
                    Severity::Error,
                    "ddl init",
                ));
            }
        }
        Ok(results)
    }
}

/// Check a single managed tool's health.
pub struct ToolCheck {
    tool: &'static Tool,
    ddl_dir: Option<DdlDir>,
}

impl ToolCheck {
    pub fn new(tool: &'static Tool, ddl_dir: Option<DdlDir>) -> Self {
        Self { tool, ddl_dir }
    }
}

impl DoctorCheck for ToolCheck {
    fn name(&self) -> &'static str {
        self.tool.name
    }
    fn description(&self) -> &'static str {
        self.tool.description
    }
    fn run(
        &self,
        _repo_root: &Path,
    ) -> std::result::Result<Vec<LintResult>, Box<dyn std::error::Error>> {
        let mut results = Vec::new();
        let installed = which(self.tool.name).is_some();

        if !installed {
            results.push(LintResult::with_fix(
                format!("{} not installed", self.tool.name),
                Severity::Error,
                format!("ddl install {}", self.tool.name),
            ));
            return Ok(results);
        }

        // Config check — untracked tools are a real warning
        if let Some(d) = &self.ddl_dir
            && !d.manifest.is_installed(self.tool.name)
        {
            results.push(LintResult::new(
                format!("{} not tracked in manifest", self.tool.name),
                Severity::Warning,
            ));
        }

        // Run doctor/diagnostic subcommand. DDL-6zn.4: version lines are NOT
        // emitted as Advisory LintResults — genesis maps Advisory→Warn, which
        // is why doctor used to report `pass: 0 warn: 37` while status said
        // "10 healthy". Only real issues produce results; nested tool-doctor
        // outcomes are classified structurally (never raw envelope text).
        for cmd in &["doctor", "diagnostic", "check"] {
            let output = Command::new(self.tool.name).args([cmd, "--json"]).output();
            if let Ok(out) = output
                && out.status.success()
            {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let first_line = stdout.lines().next().unwrap_or("");
                let (severity, detail) =
                    crate::diagnostics::classify_nested_doctor(first_line, out.status.success());
                results.push(LintResult::new(detail, severity));
                return Ok(results);
            }
        }

        Ok(results)
    }
}

// ── StatusContributor implementation ──────────────────────────────────
