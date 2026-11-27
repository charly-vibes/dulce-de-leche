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

/// The repo-scope findings (DDL-852c, design D5): a namespace, not a tool.
///
/// `ddl.repo.*` checks live under this separate key with their own summary
/// so repo findings can neither drown nor reshape the tool diagnostics
/// (`diagnostics` + `summary` count tool and machine checks only). The
/// items carry the exact same `{tool, check, level, detail, fix?}` shape
/// as the tool diagnostics — error-identity parity with the human output.
#[derive(Debug, Clone, serde::Serialize, PartialEq)]
pub struct RepoFindings {
    /// Roll-up of [`Self::checks`] — the same single-source discipline.
    pub summary: DiagnosticSummary,
    pub checks: Vec<DiagnosticItem>,
}

impl RepoFindings {
    pub fn empty() -> Self {
        Self {
            summary: DiagnosticSummary {
                pass: 0,
                warn: 0,
                fail: 0,
            },
            checks: Vec::new(),
        }
    }
}

/// The `ddl doctor --json` data payload.
#[derive(Debug, Clone, serde::Serialize, PartialEq)]
pub struct DiagnosticReport {
    pub summary: DiagnosticSummary,
    pub diagnostics: Vec<DiagnosticItem>,
    /// Repo-scope `ddl.repo.*` findings (DDL-9kz.5, DDL-852c).
    pub repo: RepoFindings,
}

/// Map a genesis doctor report onto the structured diagnostics contract.
///
/// Check names that match a managed tool are attributed to that tool;
/// everything else belongs to `"ddl"`. `ddl.repo.*` items are partitioned
/// into the `repo` key with their own summary (D5) so repo findings never
/// change how tool failures are counted; both summaries are recomputed
/// from their item lists so counts can never contradict the items.
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
    let mut repo = RepoFindings::empty();
    let mut diagnostics = Vec::new();
    for check in &report.checks {
        let level = match check.status {
            genesis::doctor::CheckStatus::Pass => "pass",
            genesis::doctor::CheckStatus::Warn => "warn",
            genesis::doctor::CheckStatus::Fail => "fail",
        };
        let tool = if tool_names.contains(&check.name.as_str()) {
            check.name.clone()
        } else {
            "ddl".to_string()
        };
        let item = DiagnosticItem {
            tool,
            check: check.name.clone(),
            level: level.to_string(),
            detail: check.message.clone(),
            fix: check.fix.clone(),
        };
        if check.name.starts_with("ddl.repo.") {
            match level {
                "pass" => repo.summary.pass += 1,
                "warn" => repo.summary.warn += 1,
                _ => repo.summary.fail += 1,
            }
            repo.checks.push(item);
            continue;
        }
        match level {
            "pass" => summary.pass += 1,
            "warn" => summary.warn += 1,
            _ => summary.fail += 1,
        }
        diagnostics.push(item);
    }
    DiagnosticReport {
        summary,
        diagnostics,
        repo,
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
///
/// Parse the nested doctor's JSON first line into (severity, warnings,
/// is-json). A non-JSON line is Advisory; a JSON line with ok!=true is
/// Error; ok with warnings is Warning; otherwise Advisory.
fn classify_doctor_output(
    first_line: &str,
    exit_ok: bool,
) -> (genesis::suite_linter::Severity, usize, bool) {
    use genesis::suite_linter::Severity;
    let parsed: std::result::Result<serde_json::Value, _> = serde_json::from_str(first_line);
    let Ok(v) = parsed else {
        let sev = if exit_ok {
            Severity::Advisory
        } else {
            Severity::Error
        };
        return (sev, 0, false);
    };
    let warnings = v
        .get("warnings")
        .and_then(|w| w.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    if v.get("ok").and_then(|o| o.as_bool()) != Some(true) {
        (Severity::Error, warnings, true)
    } else if warnings > 0 {
        (Severity::Warning, warnings, true)
    } else {
        (Severity::Advisory, warnings, true)
    }
}

/// Human detail line for the classified nested-doctor outcome.
fn doctor_detail(sev: genesis::suite_linter::Severity, warnings: usize, is_json: bool) -> String {
    use genesis::suite_linter::Severity;
    match sev {
        Severity::Error => "doctor: failed".to_string(),
        Severity::Warning => format!("doctor: ok ({warnings} warnings)"),
        Severity::Advisory => {
            if is_json {
                "doctor: ok".to_string()
            } else {
                "doctor: ran".to_string()
            }
        }
    }
}

pub fn classify_nested_doctor(
    first_line: &str,
    exit_ok: bool,
) -> (genesis::suite_linter::Severity, String) {
    let (sev, warnings, is_json) = classify_doctor_output(first_line, exit_ok);
    let detail = doctor_detail(sev, warnings, is_json);
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
    runner.register(Box::new(RepoHooksPathCheck));
    runner.register(Box::new(RepoJustCiCheck));
    runner.register(Box::new(RepoReadmeFrontdoorCheck));
    // Status-pipeline checks (DDL-852c task 2.2): the record contract and
    // the semantics statements around it.
    runner.register(Box::new(RepoStatusFreshCheck));
    runner.register(Box::new(RepoBeadsBlockCheck));
    runner.register(Box::new(RepoBriefDriftGateCheck));
    runner.register(Box::new(RepoUnmanagedArtifactsCheck));
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
