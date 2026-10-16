//! `ddl init --gates` wiring — the end state users actually ask for.
//!
//! Responsibilities:
//! - lefthook.yml managed blocks: pre-commit (ah check, pretender gate,
//!   spk lint) and pre-push (ah check), inserted *inside* `commands:`,
//!   idempotent, and self-repairing against the espectacular mangle class
//!   (markers glued to content, block outside `commands:`, duplicates)
//! - .gitignore entries for tool data dirs (.testaruda/, .pretender/)
//! - beads no-db stamping in .beads/config.yaml
//! - init ordering: openspec init BEFORE ah init (ah needs openspec/)
//!
//! Read-only counterpart (DDL-6zn.5): `detect_lefthook_gates_problems`
//! inspects the on-disk wiring without mutating it — the repo-aware doctor
//! uses it as the single source of truth for "gates wired".

use crate::error::{DdlError, Result};

/// What a wiring step did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    Created,
    Updated,
    Unchanged,
}

/// lefthook marker lines, hook-specific so pre-commit and pre-push blocks
/// are independently detectable and repairable.
pub const MARKER_START_PREFIX: &str = "# ddl:managed:start (";
pub const MARKER_END_PREFIX: &str = "# ddl:managed:end (";

/// Tool data dirs that must be gitignored (beyond the .ddl/ data files
/// already handled by `dot_ddl::GITIGNORE_ENTRIES`).
pub const GITIGNORE_TOOL_DIR_ENTRIES: &[&str] = &[".testaruda/", ".pretender/"];

/// Managed pre-commit command block (4-space indent, inside `commands:`).
/// NOTE: no leading `\`-continuation — Rust strips next-line indentation there.
pub const PRE_COMMIT_BLOCK: &str = "    # ddl:managed:start (pre-commit)
    ddl-ah-check:
      run: ah check
    ddl-pretender-gate:
      run: pretender check --diff-only --mode gate
    ddl-spk-lint:
      run: spk lint openspec/specs || test $? -eq 2
    # ddl:managed:end (pre-commit)";

/// Managed pre-push command block.
pub const PRE_PUSH_BLOCK: &str = "    # ddl:managed:start (pre-push)
    ddl-ah-check:
      run: ah check
    # ddl:managed:end (pre-push)";

/// The canonical lefthook.yml template written when a repo has none
/// (DDL-fw7): managed gate blocks plus the beads hook chaining the
/// ddl.repo.bd-hooks doctor check demands — a fresh `ddl init -y --gates`
/// must leave `ddl doctor` clean with no second manual pass.
/// `{pre_commit}` / `{pre_push}` interpolate the managed blocks.
pub const CANONICAL_LEFTHOOK_TEMPLATE: &str = "# lefthook.yml — ddl-managed gates (created by `ddl init --gates`)\n# Managed blocks: markers `# ddl:managed:start/end (hook)` — edit around them.\n\npre-commit:\n  commands:\n    bd:\n      run: bd hooks run pre-commit\n{pre_commit}\n\npre-push:\n  commands:\n    bd:\n      run: bd hooks run pre-push\n{pre_push}\n";

/// Ensure lefthook gates exist in `content` (None = file missing).
/// Returns the new content and what changed.
///
/// Rules:
/// - missing file → create with both managed blocks (Created)
/// - managed block absent → insert inside `commands:` of the hook section
///   (adding `commands:` if the hook exists without one) (Updated)
/// - managed block present inside `commands:` → no-op (Unchanged)
/// - managed block present but OUTSIDE `commands:` (mangle class) →
///   remove the stray block and re-insert inside `commands:` (Updated)
pub fn ensure_lefthook_gates(content: Option<&str>) -> Result<(String, Change)> {
    const HOOKS: &[(&str, &str)] = &[
        ("pre-commit", PRE_COMMIT_BLOCK),
        ("pre-push", PRE_PUSH_BLOCK),
    ];

    let mut lines: Vec<String> = match content {
        // format! needs a literal first argument — the template is a
        // public const, so interpolate with targeted replaces instead.
        None => CANONICAL_LEFTHOOK_TEMPLATE
            .replace("{pre_commit}", PRE_COMMIT_BLOCK)
            .replace("{pre_push}", PRE_PUSH_BLOCK)
            .lines()
            .map(|l| l.to_string())
            .collect(),
        Some(text) => text.lines().map(|l| l.to_string()).collect(),
    };
    let created = content.is_none();
    let mut changed = created;

    for (hook, block) in HOOKS {
        changed |= wire_hook(&mut lines, hook, block)?;
    }

    let mut out = lines.join("\n");
    out.push('\n');
    Ok((
        out,
        if created {
            Change::Created
        } else if changed {
            Change::Updated
        } else {
            Change::Unchanged
        },
    ))
}

/// Wire one hook's managed block. Returns true if the content changed.
fn wire_hook(lines: &mut Vec<String>, hook: &str, block: &str) -> Result<bool> {
    let start_marker = format!("{MARKER_START_PREFIX}{hook})");
    let end_marker = format!("{MARKER_END_PREFIX}{hook})");

    // Locate the top-level hook section, if any.
    let hook_line = format!("{hook}:");
    let hook_pos = lines.iter().position(|l| l.trim_end() == hook_line);

    let Some(h) = hook_pos else {
        // No hook section — append one with commands: + managed block.
        if lines.iter().any(|l| l.contains(&start_marker)) {
            return Err(DdlError::Other(format!(
                "lefthook.yml has a managed block for '{hook}' but no '{hook}:' section — manual fix required"
            )));
        }
        lines.push(String::new());
        lines.push(hook_line);
        lines.push("  commands:".to_string());
        for l in block.lines() {
            lines.push(l.to_string());
        }
        return Ok(true);
    };

    // Section spans until the next top-level key (or EOF).
    let end = lines[h + 1..]
        .iter()
        .position(|l| !l.trim().is_empty() && !l.starts_with([' ', '#']) && l.contains(':'))
        .map_or(lines.len(), |off| h + 1 + off);

    let section = &lines[h..end];
    let marker_start = section
        .iter()
        .position(|l| l.contains(&start_marker))
        .map(|i| h + i);
    let marker_end = section
        .iter()
        .position(|l| l.contains(&end_marker))
        .map(|i| h + i);

    // Managed block already present *inside* commands:? → no-op.
    // `commands:` must appear in the section before the marker start.
    if let (Some(s), Some(_e)) = (marker_start, marker_end) {
        let commands_before = section
            .iter()
            .take(s - h)
            .any(|l| l.trim_end() == "  commands:");
        if commands_before {
            return Ok(false);
        }
        // Mangle class: markers glued outside `commands:` — remove the stray
        // block and re-insert properly. Malformed markers (no end) are an error.
        // (e is always Some here: marker_end was already checked.)
        let e = marker_end.expect("end marker checked above");
        lines.drain(s..=e);
    } else if marker_start.is_some() ^ marker_end.is_some() {
        return Err(DdlError::Other(format!(
            "lefthook.yml managed block for '{hook}' has mismatched start/end markers — manual fix required"
        )));
    }

    // Re-locate section (drain may have shifted indices) and insert.
    let end = lines[h + 1..]
        .iter()
        .position(|l| !l.trim().is_empty() && !l.starts_with([' ', '#']) && l.contains(':'))
        .map_or(lines.len(), |off| h + 1 + off);
    let commands_pos = lines[h..end]
        .iter()
        .position(|l| l.trim_end() == "  commands:")
        .map(|i| h + i);

    match commands_pos {
        Some(c) => {
            let block_lines: Vec<String> = block.lines().map(|l| l.to_string()).collect();
            let insert_at = c + 1;
            for (off, l) in block_lines.iter().enumerate() {
                lines.insert(insert_at + off, l.clone());
            }
        }
        None => {
            // Hook exists without `commands:` — add it right after the hook line.
            let block_lines: Vec<String> = block.lines().map(|l| l.to_string()).collect();
            let insert_at = h + 1;
            lines.insert(insert_at, "  commands:".to_string());
            for (off, l) in block_lines.iter().enumerate() {
                lines.insert(insert_at + 1 + off, l.clone());
            }
        }
    }
    Ok(true)
}

/// Ensure tool data dir entries are in the .gitignore `content`
/// (None = file missing). Returns new content and the entries actually added.
/// Idempotent: entries already present are not duplicated.
pub fn ensure_gitignore_tool_dirs(content: Option<&str>) -> (String, Vec<String>) {
    let existing: Vec<String> = content
        .unwrap_or("")
        .lines()
        .map(|l| l.trim().to_string())
        .collect();
    let to_add: Vec<String> = GITIGNORE_TOOL_DIR_ENTRIES
        .iter()
        .filter(|e| !existing.contains(&e.to_string()))
        .map(|e| e.to_string())
        .collect();
    if to_add.is_empty() {
        // Nothing to add — content is returned byte-identical.
        return (content.unwrap_or_default().to_string(), to_add);
    }
    let mut out = String::new();
    if let Some(text) = content {
        out.push_str(text);
        if !out.ends_with('\n') {
            out.push('\n');
        }
    }
    out.push_str("\n# tool data dirs (managed by `ddl init --gates`)\n");
    for e in &to_add {
        out.push_str(e);
        out.push('\n');
    }
    (out, to_add)
}

/// Beads no-db stamp outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BeadsChange {
    Created,
    FlippedToNoDb,
    Unchanged,
}

/// Stamp `no-db: true` into .beads/config.yaml `content` (None = missing).
/// - missing → created with no-db: true
/// - present with `no-db: false` → flipped to true
/// - present with `no-db: true` → unchanged
pub fn ensure_beads_nodb(content: Option<&str>) -> (String, BeadsChange) {
    match content {
        None => (
            "# Beads Configuration File\n\
             # Stamped by `ddl init --gates`. no-db keeps the issue store in\n\
             # .beads/issues.jsonl; on bd 1.x Dolt remains the storage backend\n\
             # and agents must never run dolt commands.\nno-db: true\n"
                .to_string(),
            BeadsChange::Created,
        ),
        Some(text) => {
            let key_present = text.lines().any(|l| l.trim_start().starts_with("no-db:"));
            if !key_present {
                let mut out = text.to_string();
                if !out.ends_with('\n') {
                    out.push('\n');
                }
                out.push_str("# stamped by `ddl init --gates`\nno-db: true\n");
                return (out, BeadsChange::FlippedToNoDb);
            }
            let mut out = String::new();
            let mut flipped = false;
            for line in text.lines() {
                if line.trim_start().starts_with("no-db:") {
                    let value = line.split(':').nth(1).unwrap_or("").trim();
                    if value == "true" {
                        out.push_str(line);
                    } else {
                        out.push_str("no-db: true");
                        flipped = true;
                    }
                } else {
                    out.push_str(line);
                }
                out.push('\n');
            }
            (
                out,
                if flipped {
                    BeadsChange::FlippedToNoDb
                } else {
                    BeadsChange::Unchanged
                },
            )
        }
    }
}

/// Snapshot of the beads export state `apply_gates_wiring` needs to decide
/// whether a pre-flip export is required (DDLBeads: the funkalization trap —
/// stamping `no-db: true` over an embedded-Dolt repo with no export makes
/// every existing issue invisible to bd).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BeadsExportInput {
    /// `.beads/embeddeddolt/` exists (a Dolt database holds issues).
    pub embeddeddolt_exists: bool,
    /// `.beads/issues.jsonl` exists.
    pub issues_jsonl_exists: bool,
}

/// True when the repo has beads data in a Dolt database but no JSONL export:
/// the export must happen BEFORE `no-db: true` is stamped, otherwise bd
/// (reading only the JSONL in no-db mode) reports an empty tracker.
pub fn beads_export_needs_export(input: &BeadsExportInput) -> bool {
    input.embeddeddolt_exists && !input.issues_jsonl_exists
}

/// Summary of the on-disk gates wiring for one run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WiringReport {
    pub lefthook: Change,
    pub gitignore_added: Vec<String>,
    pub beads: BeadsChange,
    /// `bd export -o .beads/issues.jsonl` was run (pre-flip data rescue).
    pub beads_exported: bool,
    /// Untracked beads files were staged with `git add -f` (the global
    /// `~/.gitignore` excludes `.beads/`, so a plain add silently no-ops).
    pub beads_staged: Vec<String>,
}

/// Apply the full gates wiring under `repo_root`:
/// lefthook.yml managed blocks, .gitignore tool-data entries, beads no-db.
/// All steps are idempotent; errors are fatal (the user asked for gates).
pub fn apply_gates_wiring(repo_root: &std::path::Path) -> Result<WiringReport> {
    use std::fs;

    let read = |p: &std::path::Path| -> Result<Option<String>> {
        if p.exists() {
            Ok(Some(fs::read_to_string(p).map_err(DdlError::from)?))
        } else {
            Ok(None)
        }
    };
    let write = |p: &std::path::Path, content: &str| -> Result<()> {
        fs::write(p, content).map_err(DdlError::from)
    };

    // 1. lefthook.yml
    let lefthook_path = repo_root.join("lefthook.yml");
    let (lefthook_out, lefthook) = ensure_lefthook_gates(read(&lefthook_path)?.as_deref())?;
    write(&lefthook_path, &lefthook_out)?;

    // 2. .gitignore tool data dirs
    let gitignore_path = repo_root.join(".gitignore");
    let (gitignore_out, gitignore_added) =
        ensure_gitignore_tool_dirs(read(&gitignore_path)?.as_deref());
    if !gitignore_added.is_empty() {
        write(&gitignore_path, &gitignore_out)?;
    }

    // 3. beads export guard + no-db stamping (creates .beads/ if absent)
    let beads_dir = repo_root.join(".beads");
    let beads_path = beads_dir.join("config.yaml");
    let export_input = BeadsExportInput {
        embeddeddolt_exists: beads_dir.join("embeddeddolt").is_dir(),
        issues_jsonl_exists: beads_dir.join("issues.jsonl").is_file(),
    };
    let mut beads_exported = false;
    if beads_export_needs_export(&export_input) {
        // Best-effort: bd may be missing or broken; failure must not abort
        // wiring — the doctor's beads-export check re-flags it either way.
        beads_exported = run_bd_export(repo_root);
    }
    let (beads_out, beads) = ensure_beads_nodb(read(&beads_path)?.as_deref());
    if beads != BeadsChange::Unchanged {
        if let Some(parent) = beads_path.parent() {
            fs::create_dir_all(parent).map_err(DdlError::from)?;
        }
        write(&beads_path, &beads_out)?;
    }

    // 4. stage the JSONL export (and config) so the tracked-export invariant
    // holds — `~/.gitignore` commonly excludes `.beads/`, hence `-f`.
    let beads_staged = stage_beads_files(repo_root);

    Ok(WiringReport {
        lefthook,
        gitignore_added,
        beads,
        beads_exported,
        beads_staged,
    })
}

/// Run `bd export -o .beads/issues.jsonl` in `repo_root` (best-effort).
/// Returns true only when the command ran and exited successfully.
fn run_bd_export(repo_root: &std::path::Path) -> bool {
    let Some(bd) = find_in_path("bd") else {
        return false;
    };
    std::process::Command::new(bd)
        .args(["export", "-o", ".beads/issues.jsonl"])
        .current_dir(repo_root)
        .stdin(std::process::Stdio::null())
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Stage `.beads/issues.jsonl` (+ config, .gitignore) with `git add -f` when
/// they exist but are untracked. Best-effort: returns the files successfully
/// staged; failures (no git repo, no changes) yield an empty Vec.
fn stage_beads_files(repo_root: &std::path::Path) -> Vec<String> {
    let candidates = ["issues.jsonl", "config.yaml", ".gitignore"];
    let existing: Vec<String> = candidates
        .iter()
        .filter(|f| repo_root.join(".beads").join(**f).is_file())
        .map(|f| format!(".beads/{f}"))
        .collect();
    if existing.is_empty() {
        return Vec::new();
    }
    let tracked = std::process::Command::new("git")
        .args(["ls-files"])
        .args(&existing)
        .current_dir(repo_root)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();
    let untracked: Vec<&String> = existing
        .iter()
        .filter(|f| !tracked.lines().any(|t| t.trim() == **f))
        .collect();
    if untracked.is_empty() {
        return Vec::new();
    }
    let mut cmd = std::process::Command::new("git");
    cmd.args(["add", "-f"]);
    for f in &untracked {
        cmd.arg(f);
    }
    cmd.current_dir(repo_root);
    if cmd.output().map(|o| o.status.success()).unwrap_or(false) {
        untracked.iter().map(|s| (*s).clone()).collect()
    } else {
        Vec::new()
    }
}

/// Locate an executable on PATH (minimal, no external crates).
pub fn find_in_path(name: &str) -> Option<std::path::PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|p| p.is_file())
}

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

#[cfg(test)]
mod tests {
    use super::*;

    const COMMANDS_HOOK: &str = "\
pre-commit:
  parallel: true
  commands:
    bd:
      run: bd hooks run pre-commit
    fmt-check:
      glob: \"*.rs\"
      run: just fmt-check

pre-push:
  commands:
    bd:
      run: bd hooks run pre-push
";

    #[test]
    fn missing_lefthook_yml_is_created_with_both_managed_blocks() {
        let (out, change) = ensure_lefthook_gates(None).unwrap();
        assert_eq!(change, Change::Created);
        assert!(out.contains("pre-commit:"));
        assert!(out.contains("pre-push:"));
        assert!(out.contains(PRE_COMMIT_BLOCK));
        assert!(out.contains(PRE_PUSH_BLOCK));
        // DDL-fw7: the created file must already chain the beads hooks —
        // `ddl init -y --gates` then `ddl doctor` must be clean for
        // ddl.repo.bd-hooks without a second manual pass.
        assert!(out.contains("bd hooks run pre-commit"));
        assert!(out.contains("bd hooks run pre-push"));
        // managed entries must live INSIDE commands: (indent > 2)
        for line in out.lines() {
            if line.contains("ddl-ah-check") || line.contains("ddl-pretender-gate") {
                assert!(
                    line.starts_with("    "),
                    "managed entry not indented under commands:: {line:?}"
                );
            }
        }
    }

    #[test]
    fn block_is_inserted_inside_commands_preserving_existing_commands() {
        let (out, change) = ensure_lefthook_gates(Some(COMMANDS_HOOK)).unwrap();
        assert_eq!(change, Change::Updated);
        // existing commands preserved
        assert!(out.contains("bd hooks run pre-commit"));
        assert!(out.contains("just fmt-check"));
        // managed block after the pre-commit commands: line, before existing entries
        let cmds_pos = out.find("  commands:").unwrap();
        let managed_pos = out.find(MARKER_START_PREFIX).unwrap();
        let bd_pos = out.find("bd hooks run pre-commit").unwrap();
        assert!(cmds_pos < managed_pos && managed_pos < bd_pos);
        assert!(out.contains(PRE_COMMIT_BLOCK));
        assert!(out.contains(PRE_PUSH_BLOCK));
    }

    #[test]
    fn rerun_is_idempotent_no_duplicates() {
        let (once, _) = ensure_lefthook_gates(Some(COMMANDS_HOOK)).unwrap();
        let (twice, change) = ensure_lefthook_gates(Some(&once)).unwrap();
        assert_eq!(change, Change::Unchanged);
        assert_eq!(once, twice);
        assert_eq!(twice.matches(MARKER_START_PREFIX).count(), 2); // one per hook
    }

    #[test]
    fn mangled_block_outside_commands_is_repaired() {
        // espectacular mangle class: ah writer glued the managed block
        // directly under `pre-commit:` instead of inside `commands:`
        let mangled = "\
pre-commit:
# ddl:managed:start (pre-commit)
  ah-check:
    run: ah check
# ddl:managed:end (pre-commit)

  parallel: true
  commands:
    spec-gates:
      run: spk lint openspec
";
        let (out, change) = ensure_lefthook_gates(Some(mangled)).unwrap();
        assert_eq!(change, Change::Updated);
        // stray block removed (no markers glued at top of section)
        assert!(!out.contains("\n# ddl:managed:start (pre-commit)\n"));
        // repaired inside commands:, existing commands kept
        let cmds_pos = out.find("  commands:").unwrap();
        let managed_pos = out.find(MARKER_START_PREFIX).unwrap();
        assert!(cmds_pos < managed_pos);
        assert!(out.contains("run: pretender check --diff-only --mode gate"));
        assert!(out.contains("run: spk lint openspec/specs || test $? -eq 2"));
        assert!(out.contains(PRE_COMMIT_BLOCK));
        // exactly one pre-commit marker pair
        assert_eq!(out.matches("(pre-commit)").count(), 2); // start + end
    }

    #[test]
    fn hook_without_commands_gets_commands_added() {
        let bare = "pre-commit:\n  parallel: true\n";
        let (out, change) = ensure_lefthook_gates(Some(bare)).unwrap();
        assert_eq!(change, Change::Updated);
        let cmds_pos = out.find("  commands:").unwrap();
        let managed_pos = out.find(MARKER_START_PREFIX).unwrap();
        assert!(cmds_pos < managed_pos);
        assert!(out.contains(PRE_COMMIT_BLOCK));
    }

    #[test]
    fn gitignore_entries_added_idempotently() {
        let (out, added) = ensure_gitignore_tool_dirs(None);
        assert_eq!(
            added,
            GITIGNORE_TOOL_DIR_ENTRIES
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        );
        assert!(out.contains(".testaruda/"));

        let existing = "target/\n.ddl/**/*.db\n";
        let (out2, added2) = ensure_gitignore_tool_dirs(Some(existing));
        assert_eq!(added2.len(), 2);
        assert!(out2.starts_with("target/")); // existing content preserved

        let (out3, added3) = ensure_gitignore_tool_dirs(Some(&out));
        assert!(added3.is_empty(), "re-run must not duplicate: {added3:?}");
        assert_eq!(out3, out);
    }

    #[test]
    fn wiring_report_reports_export_and_staging_fields() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".beads/embeddeddolt")).unwrap();
        std::fs::write(
            dir.path().join(".beads/config.yaml"),
            "# Beads Configuration File\nno-db: false\n",
        )
        .unwrap();
        // A JSONL pre-seeded so no bd subprocess result leaks into the assert.
        std::fs::write(dir.path().join(".beads/issues.jsonl"), "{}").unwrap();
        // No git repo here → staging is a no-op (best-effort).
        let report = apply_gates_wiring(dir.path()).unwrap();
        assert_eq!(report.beads, BeadsChange::FlippedToNoDb);
        assert!(!report.beads_exported, "jsonl already present → no export");
        assert!(
            report.beads_staged.is_empty(),
            "no git repo → nothing staged"
        );
        let config = std::fs::read_to_string(dir.path().join(".beads/config.yaml")).unwrap();
        assert!(config.contains("no-db: true"));
    }

    #[test]
    fn beads_nodb_created_flipped_and_stable() {
        let (out, change) = ensure_beads_nodb(None);
        assert_eq!(change, BeadsChange::Created);
        assert!(out.contains("no-db: true"));

        let (out2, change2) = ensure_beads_nodb(Some("# Beads Configuration File\nno-db: false\n"));
        assert_eq!(change2, BeadsChange::FlippedToNoDb);
        assert!(out2.contains("no-db: true"));
        assert!(!out2.contains("no-db: false"));

        let (out3, change3) = ensure_beads_nodb(Some("# Beads Configuration File\nno-db: true\n"));
        assert_eq!(change3, BeadsChange::Unchanged);
        assert_eq!(out3, "# Beads Configuration File\nno-db: true\n");
    }

    #[test]
    fn detect_beads_export_problems_covers_invariant() {
        // No .beads at all → healthy (nothing to check).
        let base = BeadsExportProblemsInput {
            beads_dir_exists: false,
            ..Default::default()
        };
        assert!(detect_beads_export_problems(&base).is_empty());

        // Healthy no-db repo: tracked, fresh export.
        let healthy = BeadsExportProblemsInput {
            beads_dir_exists: true,
            config_exists: true,
            config_no_db: true,
            embeddeddolt_exists: false,
            issues_jsonl_exists: true,
            issues_jsonl_tracked: Some(true),
            issues_jsonl_lines: Some(41),
            bd_count: Some(41),
        };
        assert!(detect_beads_export_problems(&healthy).is_empty());

        // db-mode config without the flip.
        let db_mode = BeadsExportProblemsInput {
            beads_dir_exists: true,
            config_exists: true,
            config_no_db: false,
            ..healthy
        };
        let p = detect_beads_export_problems(&db_mode);
        assert!(p.iter().any(|m| m.contains("no-db mode")), "{p:?}");

        // DB with data but no export (funkalization trap).
        let no_export = BeadsExportProblemsInput {
            beads_dir_exists: true,
            embeddeddolt_exists: true,
            issues_jsonl_exists: false,
            issues_jsonl_tracked: None,
            config_exists: true,
            config_no_db: true,
            ..Default::default()
        };
        let p = detect_beads_export_problems(&no_export);
        assert!(
            p.iter().any(|m| m.contains("issues.jsonl is missing")),
            "{p:?}"
        );

        // Export present but untracked (gently trap).
        let untracked = BeadsExportProblemsInput {
            beads_dir_exists: true,
            config_exists: true,
            config_no_db: true,
            issues_jsonl_exists: true,
            issues_jsonl_tracked: Some(false),
            ..healthy
        };
        let p = detect_beads_export_problems(&untracked);
        assert!(p.iter().any(|m| m.contains("not tracked")), "{p:?}");

        // Stale export (uarup/titi/XAct trap).
        let stale = BeadsExportProblemsInput {
            beads_dir_exists: true,
            embeddeddolt_exists: false,
            config_exists: true,
            config_no_db: true,
            issues_jsonl_exists: true,
            issues_jsonl_tracked: Some(true),
            issues_jsonl_lines: Some(3),
            bd_count: Some(7),
        };
        let p = detect_beads_export_problems(&stale);
        assert!(p.iter().any(|m| m.contains("stale")), "{p:?}");

        // bd absent → staleness skipped, other checks still run.
        let no_bd = BeadsExportProblemsInput {
            beads_dir_exists: true,
            embeddeddolt_exists: false,
            config_exists: true,
            config_no_db: true,
            issues_jsonl_exists: true,
            issues_jsonl_tracked: Some(true),
            issues_jsonl_lines: Some(3),
            bd_count: None,
        };
        assert!(detect_beads_export_problems(&no_bd).is_empty());
    }

    #[test]
    fn beads_export_needs_export_only_when_db_exists_and_jsonl_missing() {
        let input = |db: bool, jsonl: bool| BeadsExportInput {
            embeddeddolt_exists: db,
            issues_jsonl_exists: jsonl,
        };
        // DB with no export → the funkalization trap: flipping no-db would
        // hide every issue. Export must happen first.
        assert!(beads_export_needs_export(&input(true, false)));
        // No DB (pure no-db repo or pre-init) → nothing to export.
        assert!(!beads_export_needs_export(&input(false, false)));
        assert!(!beads_export_needs_export(&input(false, true)));
        // Export already present → no-op.
        assert!(!beads_export_needs_export(&input(true, true)));
    }

    #[test]
    fn detect_flags_missing_file_and_clean_wiring() {
        assert_eq!(
            detect_lefthook_gates_problems(None),
            vec!["lefthook.yml not found — run `ddl init --gates`"]
        );
        // The fixture fixture has the managed blocks wired by hand? No — the
        // plain COMMANDS_HOOK fixture has no managed blocks at all.
        let problems = detect_lefthook_gates_problems(Some(COMMANDS_HOOK));
        assert_eq!(problems.len(), 2, "both hooks missing: {problems:?}");
        assert!(problems[0].contains("pre-commit managed block missing"));
        assert!(problems[1].contains("pre-push managed block missing"));

        // Fully wired (idempotent output of ensure) → no problems.
        let (wired, _) = ensure_lefthook_gates(Some(COMMANDS_HOOK)).unwrap();
        assert!(detect_lefthook_gates_problems(Some(&wired)).is_empty());

        // A file that merely exists but has no managed blocks still counts
        // as unwired — existence alone is not "gates wired".
        let bare_header = "# lefthook.yml — ddl-managed gates (created by `ddl init --gates`)\n";
        assert_eq!(
            detect_lefthook_gates_problems(Some(bare_header)).len(),
            2,
            "both hooks flagged"
        );
    }

    #[test]
    fn detect_flags_mangled_duplicate_and_mismatched_blocks() {
        // Block glued outside commands: (espectacular mangle class)
        let mangled = "pre-commit:\n# ddl:managed:start (pre-commit)\n  ah-check:\n    run: ah check\n# ddl:managed:end (pre-commit)\n\n  commands:\n    spec-gates:\n      run: spk lint openspec\n";
        let problems = detect_lefthook_gates_problems(Some(mangled));
        assert!(
            problems.iter().any(|p| p.contains("outside commands:")),
            "mangled block flagged: {problems:?}"
        );
        assert!(
            problems
                .iter()
                .any(|p| p.contains("pre-push managed block missing"))
        );

        // Duplicate blocks
        let (once, _) = ensure_lefthook_gates(Some(COMMANDS_HOOK)).unwrap();
        let duplicated = format!("{once}\n{}\n", PRE_COMMIT_BLOCK);
        let problems = detect_lefthook_gates_problems(Some(&duplicated));
        assert!(
            problems
                .iter()
                .any(|p| p.contains("pre-commit managed block duplicated")),
            "duplicate flagged: {problems:?}"
        );

        // Mismatched markers: start without end
        let mismatched = format!("{once}\n# ddl:managed:start (pre-commit)\n");
        let problems = detect_lefthook_gates_problems(Some(&mismatched));
        assert!(
            problems
                .iter()
                .any(|p| p.contains("pre-commit managed block has mismatched start/end")),
            "mismatch flagged: {problems:?}"
        );
    }

    #[test]
    fn openspec_inits_before_ah() {
        assert_eq!(tool_init_order(&["ah", "openspec"]), vec!["openspec", "ah"]);
        assert_eq!(tool_init_order(&["ah"]), vec!["ah"]);
        assert_eq!(
            tool_init_order(&["bd", "ah", "openspec", "wai"]),
            vec!["bd", "openspec", "ah", "wai"]
        );
        assert!(tool_init_order(&[]).is_empty());
    }
}
