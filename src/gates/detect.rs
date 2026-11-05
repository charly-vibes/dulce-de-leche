//! Doctor-side gates problem detection and tool-init ordering.

use super::*;

/// Input snapshot for [`detect_beads_export_problems`] (doctor-side, pure).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BeadsExportProblemsInput {
    pub beads_dir_exists: bool,
    pub config_exists: bool,
    pub config_no_db: bool,
    pub embeddeddolt_exists: bool,
    pub issues_jsonl_exists: bool,
    /// `None` = unknown (not a git repo / git failed).
    pub issues_jsonl_tracked: Option<bool>,
    pub issues_jsonl_lines: Option<usize>,
    /// `None` = bd missing or errored — staleness check skipped.
    pub bd_count: Option<u64>,
}

/// Doctor findings for the tracked-export invariant (DDLBeads):
/// `.beads/issues.jsonl` must exist, be git-tracked, and not lag the DB.
/// Empty ⇒ healthy. Each finding carries its own fix command.
pub fn detect_beads_export_problems(i: &BeadsExportProblemsInput) -> Vec<String> {
    let mut problems = Vec::new();
    if !i.beads_dir_exists {
        return problems;
    }
    if i.config_exists && !i.config_no_db {
        problems.push(
            "beads config is not in no-db mode (issues.jsonl not the source of truth)".to_string(),
        );
    }
    if i.embeddeddolt_exists && !i.issues_jsonl_exists {
        problems.push(
            "beads DB holds issues but .beads/issues.jsonl is missing — a no-db flip would hide them"
                .to_string(),
        );
    }
    if i.issues_jsonl_exists && i.issues_jsonl_tracked == Some(false) {
        problems.push(
            ".beads/issues.jsonl is not tracked by git (global gitignore excludes .beads/)"
                .to_string(),
        );
    }
    if let (Some(count), Some(lines)) = (i.bd_count, i.issues_jsonl_lines)
        && count > lines as u64
    {
        problems.push(format!(
            "stale issues.jsonl export ({lines} lines vs {count} issues in the DB)"
        ));
    }
    problems
}

/// Read-only inspection of the on-disk lefthook gates wiring (DDL-6zn.5).
/// Returns one problem message per defect; an empty Vec means the gates are
/// fully wired: both managed blocks present, inside `commands:`, no
/// duplicates, no mangled markers. This is the single source of truth for
/// "gates wired" shared by `init --gates` and the repo-aware doctor.
pub fn detect_lefthook_gates_problems(content: Option<&str>) -> Vec<String> {
    const HOOKS: &[&str] = &["pre-commit", "pre-push"];
    let Some(text) = content else {
        return vec!["lefthook.yml not found — run `ddl init --gates`".to_string()];
    };
    let lines: Vec<&str> = text.lines().collect();
    let mut problems = Vec::new();
    for hook in HOOKS {
        let start_marker = format!("{MARKER_START_PREFIX}{hook})");
        let end_marker = format!("{MARKER_END_PREFIX}{hook})");
        let starts: Vec<usize> = lines
            .iter()
            .enumerate()
            .filter(|(_, l)| l.contains(&start_marker))
            .map(|(i, _)| i)
            .collect();
        let ends: Vec<usize> = lines
            .iter()
            .enumerate()
            .filter(|(_, l)| l.contains(&end_marker))
            .map(|(i, _)| i)
            .collect();
        if starts.is_empty() {
            problems.push(format!("{hook} managed block missing"));
            continue;
        }
        if starts.len() > 1 {
            problems.push(format!(
                "{hook} managed block duplicated ({} blocks)",
                starts.len()
            ));
        }
        if ends.len() != starts.len() {
            problems.push(format!(
                "{hook} managed block has mismatched start/end markers"
            ));
            continue;
        }
        // The block must sit inside the hook's `commands:` — there must be a
        // two-space `commands:` line between the hook section start and the
        // first start marker (the espectacular mangle class glues the block
        // directly under the hook line, outside commands:).
        let hook_line = format!("{hook}:");
        let Some(h) = lines.iter().position(|l| l.trim_end() == hook_line) else {
            problems.push(format!(
                "{hook} managed block present but no '{hook}:' section"
            ));
            continue;
        };
        let commands_before = lines[h..starts[0]]
            .iter()
            .any(|l| l.trim_end() == "  commands:");
        if !commands_before {
            problems.push(format!(
                "{hook} managed block is outside commands: (mangled) — run `ddl init --gates`"
            ));
        }
    }
    problems
}

/// Canonical tool-init order (always applied): openspec before ah (ah init
/// exits 1 without openspec/ — the bajan 9/28 cascade failure), everything
/// else stable.
pub fn tool_init_order<'a>(tools: &[&'a str]) -> Vec<&'a str> {
    let has_openspec = tools.contains(&"openspec");
    let has_ah = tools.contains(&"ah");
    if !has_openspec || !has_ah {
        return tools.to_vec();
    }
    // Pull `ah` out and re-insert immediately after `openspec`;
    // everything else keeps its original relative order.
    let mut ordered: Vec<&'a str> = tools.iter().copied().filter(|t| *t != "ah").collect();
    let pos = ordered
        .iter()
        .position(|t| *t == "openspec")
        .expect("openspec checked above")
        + 1;
    ordered.insert(pos, "ah");
    ordered
}
