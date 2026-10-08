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
      run: pretender check --mode gate
    ddl-spk-lint:
      run: spk lint openspec
    # ddl:managed:end (pre-commit)";

/// Managed pre-push command block.
pub const PRE_PUSH_BLOCK: &str = "    # ddl:managed:start (pre-push)
    ddl-ah-check:
      run: ah check
    # ddl:managed:end (pre-push)";

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
        None => vec![
            "# lefthook.yml — ddl-managed gates (created by `ddl init --gates`)".to_string(),
            "# Managed blocks: markers `# ddl:managed:start/end (hook)` — edit around them."
                .to_string(),
        ],
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

/// Summary of the on-disk gates wiring for one run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WiringReport {
    pub lefthook: Change,
    pub gitignore_added: Vec<String>,
    pub beads: BeadsChange,
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

    // 3. beads no-db stamping (creates .beads/ if absent)
    let beads_path = repo_root.join(".beads").join("config.yaml");
    let (beads_out, beads) = ensure_beads_nodb(read(&beads_path)?.as_deref());
    if beads != BeadsChange::Unchanged {
        if let Some(parent) = beads_path.parent() {
            fs::create_dir_all(parent).map_err(DdlError::from)?;
        }
        write(&beads_path, &beads_out)?;
    }

    Ok(WiringReport {
        lefthook,
        gitignore_added,
        beads,
    })
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
        assert!(out.contains("spk lint openspec"));
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
