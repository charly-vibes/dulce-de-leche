//! Status reporting: contributor, report assembly, formatting, summary.

use super::*;
use genesis::status::{StatusBuilder, StatusContributor, StatusItem, StatusLevel, StatusSection};
use std::path::Path;

/// Contribute ddl's aggregated health to the genesis status dashboard.
pub struct DdlStatusContributor {
    ddl_dir: Option<DdlDir>,
}

impl DdlStatusContributor {
    pub fn new(ddl_dir: Option<DdlDir>) -> Self {
        Self { ddl_dir }
    }
}

/// One status line for a managed tool: installed/on-PATH vs manifest-tracked.
fn tool_status_item(tool: &crate::platform::Tool, ddl_dir: Option<&DdlDir>) -> StatusItem {
    let installed = which(tool.name).is_some();
    let version = installed
        .then(|| get_tool_version(tool))
        .flatten()
        .unwrap_or_else(|| "?".to_string());

    let config_ok = ddl_dir.is_some_and(|d| d.manifest.is_installed(tool.name));

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

    StatusItem {
        label: tool.name.to_string(),
        value,
        level,
    }
}

/// Aggregate summary over the tool items.
fn section_summary(items: &[StatusItem]) -> String {
    if items.iter().all(|i| i.level == StatusLevel::Healthy) {
        return format!("{} tools healthy", items.len());
    }
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
}

impl StatusContributor for DdlStatusContributor {
    fn name(&self) -> &'static str {
        "ddl"
    }
    fn status(&self, _repo_root: &Path) -> std::result::Result<StatusSection, String> {
        let items: Vec<StatusItem> = crate::platform::MANAGED_TOOLS
            .iter()
            .map(|tool| tool_status_item(tool, self.ddl_dir.as_ref()))
            .collect();

        let summary = section_summary(&items);

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

// ── DDL-6zn.5: repo-scope conformance checks ─────────────────────────
//
// These answer "is THIS repo fully ddl-initialized/conformant?" — the
// question every init session answered by hand-probing (ls .ddl, git
// status). They only run inside a git repository; outside one they emit
// nothing (repo conformance is meaningless there).

/// Walk up from `start` looking for a `.git` entry — the repo root.
///
/// `pub(crate)` because the DDL-fw7 init-side repo setup writes to the
/// same boundary the `ddl.repo.*` doctor checks key off — writer and
/// checker must agree on what "the repo" is.
pub(crate) fn find_git_root(start: &Path) -> Option<std::path::PathBuf> {
    // Canonicalize first: `Path::new(".").parent()` is `Some("")`, which
    // would terminate the walk at the cwd instead of ascending.
    let mut dir = Some(std::fs::canonicalize(start).unwrap_or_else(|_| start.to_path_buf()));
    while let Some(d) = dir {
        if d.join(".git").exists() {
            return Some(d);
        }
        dir = d.parent().map(|p| p.to_path_buf());
    }
    None
}

// ── Project-status section (DDL-852c task 2.1, design D1) ─────────────
//
// `ddl status` answers BOTH tool health and repo status. The project half
// is sourced from the `.ddl/status.json` record and NEVER re-obtains feeds
// (that is `--refresh`'s and the doctor check's job): it shows the record
// plus a cheap display-time "may be stale" heuristic, labeled as such.

/// One feed's display row: `{name, observed_at, staleness}` when
/// available, `{name, status: "unavailable", detail}` when not.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct ProjectFeedStatus {
    pub name: String,
    /// When the producer was last observed (available feeds only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed_at: Option<String>,
    /// `"fresh"` | `"stale"` — the record's content-state verdict
    /// (available feeds only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub staleness: Option<String>,
    /// The literal `"unavailable"` when the producer could not answer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// The producer's error for an `unavailable` feed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// The project-status section. `Serialize` IS the `--json` `project`
/// object shape: `{ record_present, generated_at?, may_be_stale,
/// suggestion?, precedence, feeds: [...] }` — no nulls (fields absent
/// rather than null, per the cli-core envelope conventions).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct ProjectStatus {
    pub record_present: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generated_at: Option<String>,
    /// Display-time heuristic (D1): the record's `generated_at` is older
    /// than the last commit touching a tracked feed store file. The
    /// authoritative content-diff verdict lives in `ddl status --refresh`
    /// and the `ddl.repo.status-fresh` doctor check, never here.
    pub may_be_stale: bool,
    /// Why the section suggests a refresh, when it does.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
    /// In-band contract-precedence header: the live record is the source
    /// of truth (shared constant from the repo-status capability).
    pub precedence: String,
    pub feeds: Vec<ProjectFeedStatus>,
}

/// Feed store files whose tracked last-commit date feeds the heuristic.
const MAY_BE_STALE_STORES: &[&str] = &[".beads/issues.jsonl", "BRIEF.json", "BRIEF.md"];

/// The may-be-stale heuristic (D1): compare the record's `generated_at`
/// with the last commit date (`git log -1 --format=%cI`) of each feed's
/// tracked store file. Cheap (one `git log` per tracked store), and
/// labeled as a heuristic wherever it renders. Any git or parse failure
/// conservatively reads as "not stale" — this display flag never blocks.
fn may_be_stale_heuristic(generated_at: &str, repo_root: &Path) -> bool {
    use chrono::DateTime;
    let Ok(generated) = DateTime::parse_from_rfc3339(generated_at) else {
        return false;
    };
    for store in MAY_BE_STALE_STORES {
        if !genesis::git::tracked(repo_root, std::path::Path::new(store)).unwrap_or(false) {
            continue;
        }
        let out = genesis::git::run(
            repo_root,
            genesis::git::EnvPolicy::Inherit,
            &["log", "-1", "--format=%cI", "--", store],
        );
        let date = out
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_default();
        if date.is_empty() {
            continue;
        }
        if let Ok(last_commit) = DateTime::parse_from_rfc3339(&date)
            && last_commit > generated
        {
            return true;
        }
    }
    false
}

/// Build the project-status section from the status record (read-only, no
/// feed obtainment — the record is the source of truth for the display,
/// D1). No `.ddl/` dir, no record file, or a structurally broken record
/// renders the no-record path with the refresh suggestion.
pub fn build_project_status(repo_root: &Path, ddl_dir: Option<&Path>) -> ProjectStatus {
    let precedence = crate::repo_status::CONTRACT_PRECEDENCE_HEADER.to_string();
    let record = ddl_dir.and_then(|dir| {
        let path = crate::repo_status::record_path(dir);
        path.exists()
            .then(|| crate::repo_status::load_record(&path).ok())
            .flatten()
    });
    let Some(record) = record else {
        return ProjectStatus {
            record_present: false,
            generated_at: None,
            may_be_stale: false,
            suggestion: Some("ddl status --refresh".to_string()),
            precedence,
            feeds: Vec::new(),
        };
    };
    let may_be_stale = may_be_stale_heuristic(&record.generated_at, repo_root);
    let feeds = record
        .data
        .feeds
        .iter()
        .map(|feed| ProjectFeedStatus {
            name: feed.name.clone(),
            observed_at: feed.observed_at().map(str::to_string),
            staleness: feed.staleness().map(|s| match s {
                crate::repo_status::Staleness::Fresh => "fresh".to_string(),
                crate::repo_status::Staleness::Stale => "stale".to_string(),
            }),
            status: feed.status.clone(),
            detail: feed.detail.clone(),
        })
        .collect();
    ProjectStatus {
        record_present: true,
        generated_at: Some(record.generated_at),
        may_be_stale,
        suggestion: may_be_stale.then(|| "ddl status --refresh".to_string()),
        precedence,
        feeds,
    }
}
