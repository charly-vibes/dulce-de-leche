//! JSON diagnostic reports: items, summaries, nested-doctor classification.

use super::*;
use genesis::doctor::DoctorRunner;
use std::path::Path;

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

/// Register the repo-scope checks (DDL-6zn.5) on a doctor runner.
///
/// Skipped entirely outside a git repository: genesis renders an empty
/// check result as a pass entry, so registering the checks outside a repo
/// would fabricate "repo gates wired" claims in machine-scope runs. The
/// presence/absence of `ddl.repo.*` items in the report IS the signal.
pub(crate) fn register_repo_checks(runner: &mut genesis::doctor::DoctorRunner) {
    runner.register(Box::new(RepoGatesCheck));
    runner.register(Box::new(RepoAgentBlocksCheck));
    runner.register(Box::new(RepoGitignoreCheck));
    runner.register(Box::new(RepoBeadsExportCheck));
    runner.register(Box::new(RepoPinsCheck));
    runner.register(Box::new(RepoLicenseCheck));
    runner.register(Box::new(RepoFmtCheck));
    runner.register(Box::new(RepoBdHookChainCheck));
    runner.register(Box::new(RepoChangelogCheck));
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
    if find_git_root(Path::new(".")).is_some() {
        register_repo_checks(&mut runner);
    }
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
