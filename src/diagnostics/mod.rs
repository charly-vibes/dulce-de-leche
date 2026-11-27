//! Diagnostic subsystem: doctor checks, status reporting, reports.

//! Health diagnostics — subprocess communication with each managed tool.
//!
//! Uses genesis doctor/status frameworks for structured reporting.
//! Each tool's health is a genesis DoctorCheck, and the aggregated
//! status uses genesis StatusBuilder.

use std::path::Path;
use std::process::Command;

use genesis::doctor::DoctorRunner;

use crate::dot_ddl::DdlDir;
use crate::error::Result;
use crate::platform::Tool;

mod checks;
mod repo_checks;
mod report;
mod status;

pub use checks::*;
pub use repo_checks::*;
pub(crate) use report::register_repo_checks;
pub use report::*;
pub(crate) use status::find_git_root;
pub use status::{
    ProjectFeedStatus, ProjectStatus, build_project_status, build_status_report,
    format_health_line, status_summary,
};

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
/// Assemble the diagnostic runner: core checks, per-tool checks, and repo
/// checks when invoked inside a git repository.
fn build_diagnostic_runner(ddl_dir: Option<&DdlDir>) -> genesis::doctor::DoctorRunner {
    let mut runner = DoctorRunner::new(vec![
        Box::new(PlatformCheck),
        Box::new(PrerequisitesCheck),
        Box::new(DdlDirCheck::new(ddl_dir.cloned())),
    ]);
    for tool in crate::platform::MANAGED_TOOLS {
        runner.register(Box::new(ToolCheck::new(tool, ddl_dir.cloned())));
    }
    runner.register(Box::new(IncitacionesSkillsCheck));
    if find_git_root(Path::new(".")).is_some() {
        register_repo_checks(&mut runner);
    }
    runner
}

/// Format the doctor report as human-readable message lines.
///
/// Repo-scope `ddl.repo.*` checks render under their own `── Repo checks ──`
/// section (DDL-852c, design D5) with the same `{icon} {check}: {detail}` +
/// `→ Run: {fix}` lines as the tool diagnostics — error-identity parity:
/// the same check name, detail, and fix strings the `--json` items carry.
fn format_diagnostic_report(report: &genesis::doctor::DoctorReport) -> Vec<String> {
    let mut messages = Vec::new();
    let mut repo_checks = Vec::new();
    let mut tool_checks = Vec::new();
    for check in &report.checks {
        if check.name.starts_with("ddl.repo.") {
            repo_checks.push(check);
        } else {
            tool_checks.push(check);
        }
    }
    messages.push("── Diagnostics ──".to_string());
    // Summary counts TOOL checks only — repo findings roll up under the
    // repo section (D5), never into the tool diagnostics.
    let count = |status: genesis::doctor::CheckStatus| {
        tool_checks.iter().filter(|c| c.status == status).count()
    };
    messages.push(format!(
        "pass: {}  warn: {}  fail: {}",
        count(genesis::doctor::CheckStatus::Pass),
        count(genesis::doctor::CheckStatus::Warn),
        count(genesis::doctor::CheckStatus::Fail),
    ));
    messages.push(String::new());

    for check in &tool_checks {
        messages.push(format_check_entry(check));
    }

    if !repo_checks.is_empty() {
        messages.push(String::new());
        messages.push("── Repo checks ──".to_string());
        for check in &repo_checks {
            messages.push(format_check_entry(check));
        }
    }
    messages
}

/// One check entry as human lines: `icon name: detail`, then `→ Run: fix`.
/// The single renderer for both sections, so a check's identity (name,
/// detail, fix) is byte-identical wherever it appears.
fn format_check_entry(check: &genesis::doctor::CheckEntry) -> String {
    let icon = match check.status {
        genesis::doctor::CheckStatus::Pass => "✓",
        genesis::doctor::CheckStatus::Warn => "⚠",
        genesis::doctor::CheckStatus::Fail => "✗",
    };
    let mut line = format!("  {icon} {}: {}", check.name, check.message);
    if let Some(fix_cmd) = &check.fix {
        line.push_str(&format!("\n     → Run: {fix_cmd}"));
    }
    line
}

/// Apply DdlDir fixes when requested (create missing manifest, remove broken
/// symlinks, etc.) and surface those messages.
fn apply_ddl_dir_fixes(ddl_dir: Option<&DdlDir>, messages: &mut Vec<String>) -> Result<()> {
    if let Some(d) = ddl_dir.filter(|_| true) {
        let fix_messages = d.doctor(true)?;
        messages.push(String::new());
        messages.push("── Fixes ──".to_string());
        messages.extend(fix_messages);
    }
    Ok(())
}

pub fn run_full_diagnostic(ddl_dir: Option<&DdlDir>, fix: bool) -> Result<Vec<String>> {
    let runner = build_diagnostic_runner(ddl_dir);

    // Build report
    let report = runner
        .run(Path::new("."), fix)
        .map_err(|e| crate::error::DdlError::Other(e.to_string()))?;

    // Format as human-readable messages
    let mut messages = format_diagnostic_report(&report);

    if fix {
        apply_ddl_dir_fixes(ddl_dir, &mut messages)?;
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
    fn report_to_diagnostics_partitions_repo_findings() {
        // DDL-852c (design D5): ddl.repo.* findings move to the `repo` key
        // with their own summary — tool diagnostics and their counts stay
        // untouched by repo findings.
        let report = DoctorReport::new(
            "ddl",
            vec![
                CheckEntry::pass("platform", "d", "ok"),
                {
                    let mut e = CheckEntry::fail("wai", "d", "wai not installed", None);
                    e.fix = Some("ddl install wai".to_string());
                    e
                },
                CheckEntry::pass("ddl.repo.gates", "d", "no issues found"),
                CheckEntry::warn(
                    "ddl.repo.unmanaged-status-artifacts",
                    "d",
                    "unmanaged status artifacts: AGENT-SUMMARY.md",
                ),
            ],
        );
        let dr = report_to_diagnostics(&report);
        // repo findings are OUT of the tool diagnostics
        assert!(
            dr.diagnostics
                .iter()
                .all(|i| !i.check.starts_with("ddl.repo.")),
            "repo findings must not drown tool diagnostics: {:?}",
            dr.diagnostics
        );
        // tool summary counts only tool items
        assert_eq!(dr.summary.pass, 1);
        assert_eq!(dr.summary.fail, 1);
        assert_eq!(dr.summary.warn, 0);
        // repo key carries its own summary, recomputed from its items
        assert_eq!(dr.repo.checks.len(), 2);
        assert_eq!(dr.repo.summary.pass, 1);
        assert_eq!(dr.repo.summary.warn, 1);
        assert_eq!(dr.repo.summary.fail, 0);
        assert_eq!(
            dr.repo.checks[1].detail,
            "unmanaged status artifacts: AGENT-SUMMARY.md"
        );
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
