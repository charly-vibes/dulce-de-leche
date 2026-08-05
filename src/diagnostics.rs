//! Health diagnostics — subprocess communication with each managed tool.
//!
//! Uses genesis doctor/status frameworks for structured reporting.
//! Each tool's health is a genesis DoctorCheck, and the aggregated
//! status uses genesis StatusBuilder.

use std::path::Path;
use std::process::Command;

use genesis::doctor::{DoctorCheck, DoctorRunner};
use genesis::status::{StatusBuilder, StatusContributor, StatusItem, StatusLevel, StatusSection};
use genesis::suite_linter::{LintResult, Severity};

use crate::dot_ddl::DdlDir;
use crate::error::Result;
use crate::platform::Tool;

/// Run a tool's `--version` command and parse the output.
pub fn get_tool_version(tool: &Tool) -> Option<String> {
    let output = Command::new(tool.name).arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let first_line = stdout.lines().next()?;
    let parts: Vec<&str> = first_line.split_whitespace().collect();
    if parts.len() >= 2 {
        let version = if parts[0].to_lowercase() == tool.name {
            parts[1]
        } else {
            parts[0]
        };
        if version.to_lowercase() == "version" && parts.len() >= 3 {
            Some(parts[2].to_string())
        } else if version.starts_with(|c: char| c.is_ascii_digit() || c == 'v') {
            Some(version.trim_start_matches('v').to_string())
        } else {
            Some(version.to_string())
        }
    } else {
        Some(first_line.to_string())
    }
}

/// Check if a command exists on PATH.
pub fn which(cmd: &str) -> Option<std::path::PathBuf> {
    std::env::var_os("PATH").and_then(|paths| {
        for dir in std::env::split_paths(&paths) {
            let full = dir.join(cmd);
            if full.is_file() {
                return Some(full);
            }
            #[cfg(windows)]
            {
                let full_exe = dir.join(std::path::PathBuf::from(cmd).with_extension("exe"));
                if full_exe.is_file() {
                    return Some(full_exe);
                }
            }
        }
        None
    })
}

// ── DoctorCheck implementations ───────────────────────────────────────

/// Check that the platform is supported.
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
            Some(p) => Ok(vec![LintResult::new(
                format!("Platform: {} ({})", p.os, p.arch),
                Severity::Advisory,
            )]),
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
        for prereq in &["curl", "git", "cargo", "brew", "scoop"] {
            if which(prereq).is_some() {
                results.push(LintResult::new(
                    format!("{prereq} found on PATH"),
                    Severity::Advisory,
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
                results.push(LintResult::new(
                    format!(".ddl/ at {}", d.path.display()),
                    Severity::Advisory,
                ));
                if d.manifest_path().exists() {
                    let tool_count = d.manifest.tools.len();
                    let installed_count = d
                        .manifest
                        .tools
                        .values()
                        .filter(|e| e.status == "installed")
                        .count();
                    results.push(LintResult::new(
                        format!(
                            "manifest.json: {tool_count} tools tracked, {installed_count} installed"
                        ),
                        Severity::Advisory,
                    ));
                } else {
                    results.push(LintResult::new("manifest.json not found", Severity::Error));
                }
                if d.config_path().exists() {
                    results.push(LintResult::new("config.toml found", Severity::Advisory));
                } else {
                    results.push(LintResult::new(
                        "config.toml not found (created on next init)",
                        Severity::Warning,
                    ));
                }
                let broken = d.detect_broken_symlinks();
                if broken.is_empty() {
                    results.push(LintResult::new("no broken symlinks", Severity::Advisory));
                } else {
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

        let version = get_tool_version(self.tool).unwrap_or_else(|| "?".to_string());
        results.push(LintResult::new(format!("v{version}"), Severity::Advisory));

        // Config check
        if let Some(d) = &self.ddl_dir {
            let config_ok = d.manifest.is_installed(self.tool.name);
            if !config_ok {
                results.push(LintResult::new(
                    format!("{} not tracked in manifest", self.tool.name),
                    Severity::Warning,
                ));
            }
        }

        // Run doctor/diagnostic subcommand
        for cmd in &["doctor", "diagnostic", "check"] {
            let output = Command::new(self.tool.name).args([cmd, "--json"]).output();
            if let Ok(out) = output
                && out.status.success() {
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    let first_line = stdout.lines().next().unwrap_or("");
                    results.push(LintResult::new(
                        format!("doctor: {first_line}"),
                        Severity::Advisory,
                    ));
                    return Ok(results);
                }
        }

        Ok(results)
    }
}

// ── StatusContributor implementation ──────────────────────────────────

/// Contribute ddl's aggregated health to the genesis status dashboard.
pub struct DdlStatusContributor {
    ddl_dir: Option<DdlDir>,
}

impl DdlStatusContributor {
    pub fn new(ddl_dir: Option<DdlDir>) -> Self {
        Self { ddl_dir }
    }
}

impl StatusContributor for DdlStatusContributor {
    fn name(&self) -> &'static str {
        "ddl"
    }
    fn status(&self, _repo_root: &Path) -> std::result::Result<StatusSection, String> {
        let mut items = Vec::new();
        for tool in crate::platform::MANAGED_TOOLS {
            let installed = which(tool.name).is_some();
            let version = installed
                .then(|| get_tool_version(tool))
                .flatten()
                .unwrap_or_else(|| "?".to_string());

            let config_ok = self
                .ddl_dir
                .as_ref()
                .is_some_and(|d| d.manifest.is_installed(tool.name));

            let level = if !installed {
                StatusLevel::Error
            } else if !config_ok {
                StatusLevel::Warning
            } else {
                StatusLevel::Healthy
            };

            let value = if installed {
                format!("v{version}")
            } else {
                "not installed".to_string()
            };

            items.push(StatusItem {
                label: tool.name.to_string(),
                value,
                level,
            });
        }

        let summary = if items.iter().all(|i| i.level == StatusLevel::Healthy) {
            format!("{} tools healthy", items.len())
        } else {
            let errors = items
                .iter()
                .filter(|i| i.level == StatusLevel::Error)
                .count();
            let warnings = items
                .iter()
                .filter(|i| i.level == StatusLevel::Warning)
                .count();
            let healthy = items
                .iter()
                .filter(|i| i.level == StatusLevel::Healthy)
                .count();
            format!("{healthy} healthy, {warnings} warnings, {errors} errors")
        };

        let mut section = StatusSection::with_items("ddl", summary, items);
        if self.ddl_dir.is_none() {
            section = section.with_suggestion("ddl init");
        }
        Ok(section)
    }
}

// ── High-level status/doctor builders ─────────────────────────────────

/// Build a genesis status report for all managed tools.
pub fn build_status_report(ddl_dir: Option<&DdlDir>) -> genesis::status::MultiToolStatus {
    let mut builder = StatusBuilder::new();
    builder.register(Box::new(DdlStatusContributor::new(ddl_dir.cloned())));
    builder
        .build(Path::new("."))
        .unwrap_or_else(|_| genesis::status::MultiToolStatus { sections: vec![] })
}

/// Format a health line for terminal display (legacy, for cmd_status).
pub fn format_health_line(health: &StatusItem) -> String {
    let icon = match health.level {
        StatusLevel::Healthy => "✓",
        StatusLevel::Warning => "⚠",
        StatusLevel::Error => "○",
    };
    format!("  {icon} {:12}  {}", health.label, health.value)
}

/// Build a human-readable status summary (legacy, for cmd_status).
pub fn status_summary(sections: &[StatusSection]) -> String {
    let total = sections.len();
    if total == 0 {
        return "No tools tracked. Run `ddl init` to get started.".to_string();
    }

    let mut healthy = 0usize;
    let mut warnings = 0usize;
    let mut errors = 0usize;

    for section in sections {
        for item in &section.items {
            match item.level {
                StatusLevel::Healthy => healthy += 1,
                StatusLevel::Warning => warnings += 1,
                StatusLevel::Error => errors += 1,
            }
        }
    }

    if errors == 0 && warnings == 0 {
        format!("All {healthy} tools are healthy. ✓")
    } else if errors == total {
        "No tools installed. Run `ddl init` to get started.".to_string()
    } else {
        let mut parts = Vec::new();
        if healthy > 0 {
            parts.push(format!("{healthy} healthy"));
        }
        if warnings > 0 {
            parts.push(format!("{warnings} warnings"));
        }
        if errors > 0 {
            parts.push(format!("{errors} not installed"));
        }
        format!("{total} tools — {}", parts.join(", "))
    }
}

/// Run a comprehensive diagnostic using genesis DoctorRunner.
///
/// When `fix` is true, also calls `DdlDir::doctor(true)` to apply fixes
/// (create missing manifest, remove broken symlinks) and includes those
/// messages in the output.
pub fn run_full_diagnostic(ddl_dir: Option<&DdlDir>, fix: bool) -> Result<Vec<String>> {
    let runner = DoctorRunner::new(vec![
        Box::new(PlatformCheck),
        Box::new(PrerequisitesCheck),
        Box::new(DdlDirCheck::new(ddl_dir.cloned())),
    ]);

    // Add per-tool checks
    let mut runner = runner;
    for tool in crate::platform::MANAGED_TOOLS {
        runner.register(Box::new(ToolCheck::new(tool, ddl_dir.cloned())));
    }

    // Build report
    let report = runner
        .run(Path::new("."), fix)
        .map_err(|e| crate::error::DdlError::Other(e.to_string()))?;

    // Format as human-readable messages
    let mut messages = Vec::new();
    messages.push("── Diagnostics ──".to_string());
    messages.push(format!(
        "pass: {}  warn: {}  fail: {}",
        report.summary.pass, report.summary.warn, report.summary.fail
    ));
    messages.push(String::new());

    for check in &report.checks {
        let icon = match check.status {
            genesis::doctor::CheckStatus::Pass => "✓",
            genesis::doctor::CheckStatus::Warn => "⚠",
            genesis::doctor::CheckStatus::Fail => "✗",
        };
        messages.push(format!("  {icon} {}: {}", check.name, check.message));
        if let Some(ref fix_cmd) = check.fix {
            messages.push(format!("     → Run: {fix_cmd}"));
        }
    }

    // Apply DdlDir fixes when requested (create missing manifest, remove
    // broken symlinks, etc.) and surface those messages.
    if fix
        && let Some(d) = ddl_dir
    {
        let fix_messages = d.doctor(true)?;
        messages.push(String::new());
        messages.push("── Fixes ──".to_string());
        messages.extend(fix_messages);
    }

    Ok(messages)
}
