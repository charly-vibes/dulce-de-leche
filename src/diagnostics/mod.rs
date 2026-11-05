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
pub use status::{build_status_report, format_health_line, status_summary};

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
    if find_git_root(Path::new(".")).is_some() {
        register_repo_checks(&mut runner);
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
