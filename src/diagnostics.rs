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
                format!("v{version} [{}/{}]", tool.category, tool.maturity)
            } else {
                format!("not installed [{}/{}]", tool.category, tool.maturity)
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
    let total: usize = sections.iter().map(|s| s.items.len()).sum();
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
    runner.register(Box::new(IncitacionesSkillsCheck));

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
    if fix && let Some(d) = ddl_dir {
        let fix_messages = d.doctor(true)?;
        messages.push(String::new());
        messages.push("── Fixes ──".to_string());
        messages.extend(fix_messages);
    }

    Ok(messages)
}

// ── DDL-6zn.4: versioned structured diagnostics contract ────────────────

#[cfg(test)]
mod contract_tests {
    use super::*;
    use genesis::doctor::{CheckEntry, DoctorReport};

    #[test]
    fn classify_nested_doctor_is_structured_never_raw() {
        use genesis::suite_linter::Severity;
        // nested failure → Error, no raw envelope text
        let (sev, detail) = classify_nested_doctor(r#"{"ok":false}"#, false);
        assert_eq!(sev, Severity::Error);
        assert_eq!(detail, "doctor: failed");
        // nested warnings roll up as Warning with a count, not the envelope
        let (sev, detail) = classify_nested_doctor(
            r#"{"ok":true,"warnings":[{"message":"a"},{"message":"b"}]}"#,
            true,
        );
        assert_eq!(sev, Severity::Warning);
        assert_eq!(detail, "doctor: ok (2 warnings)");
        assert!(!detail.contains("warnings\":["));
        // clean nested run → advisory (pass)
        let (sev, detail) = classify_nested_doctor(r#"{"ok":true}"#, true);
        assert_eq!(sev, Severity::Advisory);
        assert_eq!(detail, "doctor: ok");
        // legacy non-JSON first line → advisory, detail stays compact
        let (sev, detail) = classify_nested_doctor("initialized: 3 specs", true);
        assert_eq!(sev, Severity::Advisory);
        assert_eq!(detail, "doctor: ran");
    }

    #[test]
    fn report_to_diagnostics_maps_items_and_rolls_up_summary() {
        let report = DoctorReport::new(
            "ddl",
            vec![
                CheckEntry::pass("platform", "d", "ok"),
                {
                    let mut e = CheckEntry::fail("wai", "d", "wai not installed", None);
                    e.fix = Some("ddl install wai".to_string());
                    e
                },
                CheckEntry::warn("bd", "d", "doctor: ok (1 warning)"),
                CheckEntry::pass("wai", "d", "no issues found"),
            ],
        );
        let dr = report_to_diagnostics(&report);
        // tool attribution: managed-tool check names → that tool, else ddl
        assert_eq!(dr.diagnostics[0].tool, "ddl");
        assert_eq!(dr.diagnostics[1].tool, "wai");
        assert_eq!(dr.diagnostics[2].tool, "bd");
        // fix surfaces as the optional field
        assert_eq!(dr.diagnostics[1].fix.as_deref(), Some("ddl install wai"));
        assert!(dr.diagnostics[0].fix.is_none());
        // summary is a roll-up of the items — single source
        assert_eq!(dr.summary.pass, 2);
        assert_eq!(dr.summary.warn, 1);
        assert_eq!(dr.summary.fail, 1);
        // levels are the lowercase strings of the contract
        assert_eq!(dr.diagnostics[1].level, "fail");
        assert_eq!(dr.diagnostics[2].level, "warn");
        assert_eq!(dr.diagnostics[0].level, "pass");
    }
}

/// One structured diagnostic item (DDL-6zn.4 contract).
///
/// JSON shape (pinned by tests/envelope_contract.rs):
/// `{ "tool", "check", "level": "pass|warn|fail", "detail", "fix"? }`
/// — `fix` is omitted, never null; no other keys; no null values.
#[derive(Debug, Clone, serde::Serialize, PartialEq)]
pub struct DiagnosticItem {
    /// Owning tool — `"ddl"` for ddl's own checks.
    pub tool: String,
    /// Check identifier (genesis check name).
    pub check: String,
    /// Severity level: `"pass"`, `"warn"`, or `"fail"`.
    pub level: String,
    /// Human-readable detail — structured, never a raw nested envelope.
    pub detail: String,
    /// Exact fix command when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix: Option<String>,
}

/// Roll-up of every item's level — the single source for summary counts.
#[derive(Debug, Clone, serde::Serialize, PartialEq)]
pub struct DiagnosticSummary {
    pub pass: usize,
    pub warn: usize,
    pub fail: usize,
}

/// The `ddl doctor --json` data payload.
#[derive(Debug, Clone, serde::Serialize, PartialEq)]
pub struct DiagnosticReport {
    pub summary: DiagnosticSummary,
    pub diagnostics: Vec<DiagnosticItem>,
}

/// Map a genesis doctor report onto the structured diagnostics contract.
///
/// Check names that match a managed tool are attributed to that tool;
/// everything else belongs to `"ddl"`. The summary is recomputed from the
/// items so counts can never contradict the item list.
pub fn report_to_diagnostics(report: &genesis::doctor::DoctorReport) -> DiagnosticReport {
    let tool_names: Vec<&str> = crate::platform::MANAGED_TOOLS
        .iter()
        .map(|t| t.name)
        .collect();
    let mut summary = DiagnosticSummary {
        pass: 0,
        warn: 0,
        fail: 0,
    };
    let mut diagnostics = Vec::new();
    for check in &report.checks {
        let level = match check.status {
            genesis::doctor::CheckStatus::Pass => "pass",
            genesis::doctor::CheckStatus::Warn => "warn",
            genesis::doctor::CheckStatus::Fail => "fail",
        };
        match level {
            "pass" => summary.pass += 1,
            "warn" => summary.warn += 1,
            _ => summary.fail += 1,
        }
        let tool = if tool_names.contains(&check.name.as_str()) {
            check.name.clone()
        } else {
            "ddl".to_string()
        };
        diagnostics.push(DiagnosticItem {
            tool,
            check: check.name.clone(),
            level: level.to_string(),
            detail: check.message.clone(),
            fix: check.fix.clone(),
        });
    }
    DiagnosticReport {
        summary,
        diagnostics,
    }
}

/// Interpret a nested tool-doctor envelope into `(severity, detail)`.
/// Never embeds the raw envelope line — that is the mangle class that made
/// 0.3-era doctor output unparseable (DDL-6zn.4).
///
/// - non-zero exit or `ok:false` → `(Error, "doctor: failed")`
/// - `ok:true` with N warnings → `(Warning, "doctor: ok (N warnings)")`
/// - `ok:true` clean → `(Advisory, "doctor: ok")`
/// - non-JSON legacy output → `(Advisory, "doctor: ran")`
pub fn classify_nested_doctor(
    first_line: &str,
    exit_ok: bool,
) -> (genesis::suite_linter::Severity, String) {
    use genesis::suite_linter::Severity;
    let sev = if !exit_ok {
        Severity::Error
    } else {
        let parsed: std::result::Result<serde_json::Value, _> = serde_json::from_str(first_line);
        match parsed {
            Ok(v) => {
                if v.get("ok").and_then(|o| o.as_bool()) != Some(true) {
                    Severity::Error
                } else if v
                    .get("warnings")
                    .and_then(|w| w.as_array())
                    .is_some_and(|a| !a.is_empty())
                {
                    Severity::Warning
                } else {
                    Severity::Advisory
                }
            }
            Err(_) => Severity::Advisory,
        }
    };
    let detail = match sev {
        Severity::Error => "doctor: failed".to_string(),
        Severity::Warning => {
            let warnings = serde_json::from_str::<serde_json::Value>(first_line)
                .ok()
                .and_then(|v| {
                    v.get("warnings")
                        .and_then(|w| w.as_array())
                        .map(|a| a.len())
                })
                .unwrap_or(0);
            format!("doctor: ok ({warnings} warnings)")
        }
        Severity::Advisory => {
            if serde_json::from_str::<serde_json::Value>(first_line).is_ok() {
                "doctor: ok".to_string()
            } else {
                "doctor: ran".to_string()
            }
        }
    };
    (sev, detail)
}

/// Build the structured `ddl doctor` report (JSON path, DDL-6zn.4).
///
/// Same checks as [`run_full_diagnostic`], but machine-shaped: items plus a
/// roll-up summary computed from those items.
pub fn run_full_report(ddl_dir: Option<&DdlDir>, fix: bool) -> Result<DiagnosticReport> {
    let mut runner = DoctorRunner::new(vec![
        Box::new(PlatformCheck),
        Box::new(PrerequisitesCheck),
        Box::new(DdlDirCheck::new(ddl_dir.cloned())),
    ]);
    for tool in crate::platform::MANAGED_TOOLS {
        runner.register(Box::new(ToolCheck::new(tool, ddl_dir.cloned())));
    }
    runner.register(Box::new(IncitacionesSkillsCheck));
    let report = runner
        .run(Path::new("."), fix)
        .map_err(|e| crate::error::DdlError::Other(e.to_string()))?;
    let mut mapped = report_to_diagnostics(&report);

    // Apply DdlDir fixes when requested (create missing manifest, remove
    // broken symlinks) — same as the human path — and surface the fix
    // messages as structured `ddl/fixes` items so JSON consumers see them.
    if fix && let Some(d) = ddl_dir {
        let fix_messages = d.doctor(true)?;
        for msg in fix_messages {
            let level = if msg.starts_with("✗") {
                "fail"
            } else {
                "pass"
            };
            if level == "fail" {
                mapped.summary.fail += 1;
            } else {
                mapped.summary.pass += 1;
            }
            mapped.diagnostics.push(DiagnosticItem {
                tool: "ddl".to_string(),
                check: "fixes".to_string(),
                level: level.to_string(),
                detail: msg.trim().to_string(),
                fix: None,
            });
        }
    }

    Ok(mapped)
}
