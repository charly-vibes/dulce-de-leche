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
