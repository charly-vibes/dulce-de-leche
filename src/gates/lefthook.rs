//! Lefthook gates wiring: managed blocks, canonical template, hook idempotent wiring, gitignore tool-dir entries.

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
