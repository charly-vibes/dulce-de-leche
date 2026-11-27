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

/// Repo-local negation of the global `.beads/` ignore (DDL-9kz.2): the
/// tracked export must be addable without `-f`, while the embedded DB dir
/// stays ignored. Repo-local .gitignore outranks the global excludesFile,
/// so `!.beads/` re-includes the directory and the later re-ignore keeps
/// the DB out. Lines appended as one managed block by
/// [`ensure_gitignore_tool_dirs`].
pub const GITIGNORE_BEADS_NEGATION: &[&str] = &["!.beads/", ".beads/embeddeddolt/"];

/// Managed pre-commit command block (4-space indent, inside `commands:`).
/// This is the default rendering — [`render_pre_commit_block`] reproduces it
/// from [`GateCommands::default`] and overlays `.ddl/config.toml` values.
/// NOTE: no leading `\`-continuation — Rust strips next-line indentation there.
///
/// The `ddl-pretender-gate` entry must scan the staged files (`--staged`); the
/// older `--diff-only` read the unstaged tree, which is empty once a commit
/// is staged, so the gate silently checked nothing (DDL-sl9).
pub const PRE_COMMIT_BLOCK: &str = "    # ddl:managed:start (pre-commit)
    ddl-ah-check:
      run: ah check
    ddl-pretender-gate:
      run: pretender check --staged --mode gate
    ddl-spk-lint:
      run: spk lint openspec/specs || test $? -eq 2
    # ddl:managed:end (pre-commit)";

/// Shipped default for the `ddl-ah-check` hook entry.
pub const DEFAULT_AH_CHECK: &str = "ah check";
/// Shipped default for the `ddl-pretender-gate` hook entry.
///
/// `--staged` (not `--diff-only`): a pre-commit hook runs after `git add`, so
/// the unstaged diff is empty by construction and `--diff-only` would scan
/// nothing and exit 0 — a silently disarmed gate (DDL-sl9). `--staged`
/// selects the staged path set but scans the **working-tree copy** of each
/// selected path: the staged blob itself is not scanned (index holds the
/// violating version, worktree the clean one — index-accurate scanning needs
/// `git show :path` support in pretender, DDL-api), and merge commits bypass
/// the gate entirely (git does not run pre-commit for `git merge`;
/// pre-merge-commit is not wired).
pub const DEFAULT_PRETENDER_GATE: &str = "pretender check --staged --mode gate";
/// Shipped default for the `ddl-spk-lint` hook entry.
pub const DEFAULT_SPK_LINT: &str = "spk lint openspec/specs || test $? -eq 2";

/// The brief-drift ordering sentence, shared with `ddl status --refresh`
/// (task 1.5). The emitted gate script embeds this constant so the gate
/// message and the refresh flow can never drift apart (design D4: one
/// source of truth for the ordering).
use crate::repo_status::STATUS_ORDERING_MESSAGE;

/// The emitted `ddl-brief-drift` pre-commit gate command (DDL-4jvz task
/// 3.1, design D6). Self-contained shell — ddl owns no external script
/// file — rendered as a YAML block scalar under `run: |`.
///
/// Semantics ("renders must be determined modulo content"): regenerate the
/// render pair into a temp dir at the committed render's own `as_of`
/// (`podras brief --write --as-of --out-dir` — verified against podras's
/// CLI; it never touches the working tree), normalize the volatile
/// envelope fields (`generated_at`, `as_of`, `cli_version`, per-source
/// `observed_at`) and the HEAD-churn sections (`## Recent`,
/// `orphan_commits`, `head_sha` — they describe the current HEAD and churn
/// on every commit; found 2026-11-25 in the dulce fork this replaces), then
/// diff. On drift the failure names the committed envelope's producing
/// podras version (`cli_version`, so a version bump is distinguishable
/// from real drift) and states the ordering. `PODRAS_BIN` overrides the
/// binary; a missing podras is an advisory skip, not a pass.
pub const DEFAULT_BRIEF_DRIFT: &str = r##"set -eu
root=$(git rev-parse --show-toplevel 2>/dev/null) || { echo "brief-drift: not inside a git repository" >&2; exit 2; }
cd "$root"
md=BRIEF.md
json=BRIEF.json
tracked() { git ls-files --error-unmatch -- "$1" >/dev/null 2>&1; }
if ! tracked "$md" && ! tracked "$json"; then
  # No render, no gate (DDL-4jvz): beads-only repos are covered by the
  # export-drift guard instead.
  exit 0
fi
if ! tracked "$md" || ! tracked "$json"; then
  echo "brief-drift: incomplete committed renders: $md and $json must exist together" >&2
  exit 1
fi
command -v jq >/dev/null 2>&1 || { echo "brief-drift: jq is required to normalize renders" >&2; exit 2; }
podras_bin=${PODRAS_BIN:-podras}
command -v "$podras_bin" >/dev/null 2>&1 || {
  echo "brief-drift: podras not found (set PODRAS_BIN) — render gate skipped, not passed" >&2
  exit 0
}
as_of=$(jq -r '(.data.as_of // (.data.generated_at | .[0:10])) // empty' "$json" 2>/dev/null) || as_of=
as_of=${as_of:-$(date -u +%F)}
tmp=$(mktemp -d "${TMPDIR:-/tmp}/ddl-brief-drift.XXXXXX")
trap 'rm -rf "$tmp"' EXIT
tree_before=$(git status --porcelain)
"$podras_bin" brief --write --as-of "$as_of" --out-dir "$tmp" >/dev/null 2>&1 || {
  echo "brief-drift: podras brief --write failed for as-of $as_of — run it manually to see the error" >&2
  exit 1
}
for f in "$md" "$json"; do
  [ -e "$tmp/$f" ] || { echo "brief-drift: regeneration produced no $f" >&2; exit 1; }
done
normalize_json() {
  jq 'del(.generated_at, .cli_version, .data.generated_at, .data.as_of, .data.cli_version, .data.source_hash, .data.warnings, .meta.author)
      | del(.data.curation.updated_at)
      | del(.data.recent)
      | del(.data.head_sha)
      | (.data.sources[]? |= del(.observed_at, .staleness, .detail))
      | (.data.health.signals = ((.data.health.signals // []) | map(select(.kind != "orphan_commits"))))
      | (.data.health.reason = ((.data.health.reason // "") | sub("orphan_commits:.*"; "orphan_commits: <head-churn omitted>")))' "$1"
}
normalize_markdown() {
  awk '
      /^## Sources & freshness/ { exit }
      /^## Recent$/ { skip = 1; next }
      /^## / { skip = 0 }
      skip { next }
      /orphan[-_ ]?commits/ { next }
      { print }
  ' "$1"
}
drift=0
for artifact in "$md" "$json"; do
  if [ "$artifact" = "$json" ]; then norm=normalize_json; else norm=normalize_markdown; fi
  "$norm" "$artifact" >"$tmp/$artifact.committed.norm"
  "$norm" "$tmp/$artifact" >"$tmp/$artifact.fresh.norm"
  diff -u --label "$artifact (committed)" --label "$artifact (regenerated)" "$tmp/$artifact.committed.norm" "$tmp/$artifact.fresh.norm" >"$tmp/$artifact.diff" || drift=$((drift + 1))
done
tree_after=$(git status --porcelain)
if [ "$tree_before" != "$tree_after" ]; then
  echo "brief-drift: the gate changed the working tree; it must never write the repository" >&2
  exit 1
fi
if [ "$drift" -gt 0 ]; then
  for artifact in "$md" "$json"; do
    if [ -s "$tmp/$artifact.diff" ]; then
      echo "brief-drift: $artifact drifted from a fresh render at $as_of" >&2
      cat "$tmp/$artifact.diff" >&2
    fi
  done
  cli_version=$(jq -r '.cli_version // .data.cli_version // empty' "$json" 2>/dev/null) || cli_version=
  if [ -n "$cli_version" ]; then
    echo "brief-drift: committed render was produced by podras $cli_version; re-running the ordering regenerates it with the installed podras, so a version bump alone is self-healing, not real drift" >&2
  fi
  # Spliced by render_brief_drift_command: the shared ordering sentence,
  # shell-single-quoted so its backticks are never command substitution.
  printf 'brief-drift: %s\n' $STATUS_ORDERING_MESSAGE >&2
  exit 1
fi
echo "brief-drift: $md and $json match a fresh render at $as_of"
"##;

/// Inline the shared ordering sentence into the gate script: the script is
/// a static string, so the sentence is spliced at render time by
/// [`GateCommands::for_repo`] when arming. The splice is shell-single-
/// quoted so backticks in the sentence are never command substitution at
/// gate runtime. The rendered command must never be built from
/// [`DEFAULT_BRIEF_DRIFT`] directly.
pub(crate) fn render_brief_drift_command() -> String {
    let quoted = format!("'{}'", STATUS_ORDERING_MESSAGE.replace('\'', r"'\''"));
    DEFAULT_BRIEF_DRIFT.replace("$STATUS_ORDERING_MESSAGE", &quoted)
}

/// Gate command lines rendered into the managed pre-commit block (DDL-1bz).
///
/// Built from [`crate::dot_ddl::GatesConfig`]; an empty string omits that
/// command id from the rendered block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateCommands {
    pub ah_check: String,
    pub pretender_gate: String,
    pub spk_lint: String,
    /// Brief-render drift gate command (`ddl-brief-drift` pre-commit
    /// entry, DDL-4jvz). Empty string omits the entry — the default, since
    /// arming is content-based: [`GateCommands::for_repo`] sets it only
    /// when the repo has a committed podras brief render.
    pub brief_drift: String,
}

impl Default for GateCommands {
    fn default() -> Self {
        Self {
            ah_check: DEFAULT_AH_CHECK.to_string(),
            pretender_gate: DEFAULT_PRETENDER_GATE.to_string(),
            spk_lint: DEFAULT_SPK_LINT.to_string(),
            brief_drift: String::new(),
        }
    }
}

impl GateCommands {
    /// Resolve the effective commands from config overrides: `None` keeps
    /// the shipped default, `Some("")` disables that entry. The brief-drift
    /// gate has no shipped default here — arming is content-based and
    /// happens in [`GateCommands::for_repo`], which has the repo root.
    pub fn from_config(config: &crate::dot_ddl::GatesConfig) -> Self {
        let pick = |value: &Option<String>, default: &str| match value {
            Some(v) => v.clone(),
            None => default.to_string(),
        };
        Self {
            ah_check: pick(&config.ah_check, DEFAULT_AH_CHECK),
            pretender_gate: pick(&config.pretender_gate, DEFAULT_PRETENDER_GATE),
            spk_lint: pick(&config.spk_lint, DEFAULT_SPK_LINT),
            brief_drift: pick(&config.brief_drift, ""),
        }
    }

    /// Load `.ddl/config.toml` under `repo_root`; missing or unreadable
    /// config falls back to the defaults.
    pub fn for_repo(repo_root: &std::path::Path) -> Self {
        use genesis::config::ConfigFile;
        let config = crate::dot_ddl::DdlConfig::read(&repo_root.join(".ddl")).unwrap_or_default();
        let mut commands = Self::from_config(&config.gates);
        if config.gates.brief_drift.is_none() {
            // Auto-arming is content-based, never timestamp-based
            // (DDL-4jvz "no render, no gate"): the gate exists only where
            // a render is committed; beads-only repos keep the
            // export-drift guard instead.
            commands.brief_drift = if Self::brief_render_tracked(repo_root) {
                render_brief_drift_command()
            } else {
                String::new()
            };
        }
        commands
    }

    /// Content-based gate-arming test (DDL-4jvz): a podras brief render is
    /// committed — BRIEF.md or BRIEF.json is git-tracked. Never
    /// timestamp-based, so a repo whose render was merely touched but not
    /// committed never arms or disarms the gate.
    pub fn brief_render_tracked(repo_root: &std::path::Path) -> bool {
        let candidates = ["BRIEF.md", "BRIEF.json"];
        let tracked: Vec<String> =
            genesis::git::tracked_paths(repo_root, &candidates).unwrap_or_default();
        candidates.iter().any(|c| {
            tracked
                .iter()
                .any(|t| std::path::Path::new(t.as_str()) == std::path::Path::new(c))
        })
    }
}

/// Render the managed pre-commit block for `commands`, in the canonical
/// shape (4-space indent, marker pair, one entry per non-empty command).
pub fn render_pre_commit_block(commands: &GateCommands) -> String {
    let mut lines = vec![format!("    {MARKER_START_PREFIX}pre-commit)")];
    let mut entry = |id: &str, run: &str| {
        if !run.is_empty() {
            lines.push(format!("    {id}:"));
            lines.push(format!("      run: {run}"));
        }
    };
    entry("ddl-ah-check", &commands.ah_check);
    entry("ddl-pretender-gate", &commands.pretender_gate);
    entry("ddl-spk-lint", &commands.spk_lint);
    // Brief-drift gate (DDL-4jvz): armed only when a brief render is
    // committed. The command is a self-contained script rendered as a YAML
    // block scalar (`run: |`) so ddl owns no external script file; drift
    // detection compares the whole span, so the scalar body must be stable
    // — no blank lines (a blank line would render as bare indentation).
    if !commands.brief_drift.is_empty() {
        lines.push("    ddl-brief-drift:".to_string());
        lines.push("      run: |".to_string());
        for l in commands.brief_drift.lines() {
            if l.is_empty() {
                lines.push(String::new());
            } else {
                lines.push(format!("        {l}"));
            }
        }
    }
    lines.push(format!("    {MARKER_END_PREFIX}pre-commit)"));
    lines.join("\n")
}

/// Managed pre-push command block (default rendering, matching
/// [`render_pre_push_block`] for the default gate commands).
pub const PRE_PUSH_BLOCK: &str = "    # ddl:managed:start (pre-push)
    ddl-ah-check:
      run: ah check
    # ddl:managed:end (pre-push)";

/// Render the managed pre-push block for `commands`. The pre-push gate runs
/// the same `ah_check` command as pre-commit (repos with a ratchet wrapper
/// must gate pushes on it too); an empty value omits the entry.
pub fn render_pre_push_block(commands: &GateCommands) -> String {
    let mut lines = vec![format!("    {MARKER_START_PREFIX}pre-push)")];
    if !commands.ah_check.is_empty() {
        lines.push("    ddl-ah-check:".to_string());
        lines.push(format!("      run: {}", commands.ah_check));
    }
    lines.push(format!("    {MARKER_END_PREFIX}pre-push)"));
    lines.join("\n")
}

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
/// - managed block present inside `commands:` → no-op (Unchanged) when its
///   text matches `commands`; stale text is re-rendered (Updated)
/// - managed block present but OUTSIDE `commands:` (mangle class) →
///   remove the stray block and re-insert inside `commands:` (Updated)
pub fn ensure_lefthook_gates(
    content: Option<&str>,
    commands: &GateCommands,
) -> Result<(String, Change)> {
    let pre_commit = render_pre_commit_block(commands);
    let pre_push = render_pre_push_block(commands);
    let hooks: &[(&str, &str)] = &[
        ("pre-commit", pre_commit.as_str()),
        ("pre-push", pre_push.as_str()),
    ];

    let mut lines: Vec<String> = match content {
        // format! needs a literal first argument — the template is a
        // public const, so interpolate with targeted replaces instead.
        None => CANONICAL_LEFTHOOK_TEMPLATE
            .replace("{pre_commit}", &pre_commit)
            .replace("{pre_push}", &pre_push)
            .lines()
            .map(|l| l.to_string())
            .collect(),
        Some(text) => text.lines().map(|l| l.to_string()).collect(),
    };
    let created = content.is_none();
    let mut changed = created;

    for (hook, block) in hooks {
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

    // Managed block already present *inside* commands:? → compare text.
    // `commands:` must appear in the section before the marker start.
    if let (Some(s), Some(e)) = (marker_start, marker_end) {
        if e < s {
            return Err(DdlError::Other(format!(
                "lefthook.yml managed block for '{hook}' has its end marker before its start marker — manual fix required"
            )));
        }
        let commands_before = section
            .iter()
            .take(s - h)
            .any(|l| l.trim_end() == "  commands:");
        if commands_before {
            // DDL-1bz: a stale span (old command text, hand-edited line) is
            // drift, not a no-op — re-render it from the effective config.
            if lines[s..=e].join("\n").trim_end() == block.trim_end() {
                return Ok(false);
            }
            let block_lines: Vec<String> = block.lines().map(|l| l.to_string()).collect();
            lines.splice(s..=e, block_lines);
            return Ok(true);
        }
        // Mangle class: markers glued outside `commands:` — remove the stray
        // block and re-insert properly. Malformed markers (no end) are an error.
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
    let mut to_add: Vec<String> = GITIGNORE_TOOL_DIR_ENTRIES
        .iter()
        .filter(|e| !existing.contains(&e.to_string()))
        .map(|e| e.to_string())
        .collect();
    // Beads tracked-export negation (DDL-9kz.2): a whole-block check, not
    // per-line — either the block is absent (both lines added) or the
    // negation is already in place (nothing added). A partial block
    // (negation present, DB re-ignore absent) is not produced by ddl and
    // must not be "fixed" by duplicating lines.
    let negation_absent = !existing.contains(&"!.beads/".to_string());
    if negation_absent {
        to_add.extend(GITIGNORE_BEADS_NEGATION.iter().map(|e| e.to_string()));
    }
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
        if e == "!.beads/" {
            out.push_str("\n# ddl: tracked beads export negates the global ignore (DDL-9kz.2)\n");
        }
        out.push_str(e);
        out.push('\n');
    }
    (out, to_add)
}
