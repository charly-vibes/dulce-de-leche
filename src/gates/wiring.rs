//! One-shot gates wiring (ddl init --gates): lefthook, gitignore, beads, export staging.

use super::*;
use crate::error::{DdlError, Result};

/// Outcome of the sad-gate wiring step (sad-8ov): ddl invokes `sad hooks
/// install` and reports the result — it never edits sad-owned markers
/// itself (boundary rule: sad owns the marker and the gate command).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SadGateOutcome {
    /// `sad hooks install` ran and exited 0; the note is its captured output.
    Wired(String),
    /// `sad` is not on PATH — advisory skip, nothing wired.
    NotInstalled,
    /// `sad hooks install` refused (no git repo, no lefthook config, unusable
    /// config); the note carries sad's message. Advisory: wiring stays green.
    Refused(String),
}

/// Summary of the on-disk gates wiring for one run.
#[derive(Debug, Clone, PartialEq, Eq)]

pub struct WiringReport {
    pub lefthook: Change,
    /// What the sad-gate step did (invoke `sad hooks install` or skip).
    pub sad: SadGateOutcome,
    pub gitignore_added: Vec<String>,
    pub beads: BeadsChange,
    /// `bd export -o .beads/issues.jsonl` was run (pre-flip data rescue).
    pub beads_exported: bool,
    /// Untracked beads files were staged with `git add -f` (the global
    /// `~/.gitignore` excludes `.beads/`, so a plain add silently no-ops).
    pub beads_staged: Vec<String>,
    /// What the session-close protocol block step did (DDL-9kz.1).
    pub session_protocol: crate::installer::SessionProtocolChange,
}

/// Apply the full gates wiring under `repo_root`:
/// lefthook.yml managed blocks, .gitignore tool-data entries, beads no-db.
/// All steps are idempotent; errors are fatal (the user asked for gates).
pub fn apply_gates_wiring(repo_root: &std::path::Path) -> Result<WiringReport> {
    apply_gates_wiring_with_sad(repo_root, find_in_path("sad").as_deref())
}

/// Same wiring with the `sad` binary injected (test seam): `None` emulates
/// sad being absent, keeping the sad step deterministic without PATH races.
pub fn apply_gates_wiring_with_sad(
    repo_root: &std::path::Path,
    sad_bin: Option<&std::path::Path>,
) -> Result<WiringReport> {
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

    // 1. lefthook.yml — render from the repo's `.ddl/config.toml` gates
    //    config (defaults when absent), so stale spans are rewritten (DDL-1bz).
    let commands = GateCommands::for_repo(repo_root);
    let lefthook_path = repo_root.join("lefthook.yml");
    let (lefthook_out, lefthook) =
        ensure_lefthook_gates(read(&lefthook_path)?.as_deref(), &commands)?;
    write(&lefthook_path, &lefthook_out)?;

    // 1b. sad gate — sad owns its block markers and gate command (boundary
    // rule), so ddl never renders a sad block itself: it invokes
    // `sad hooks install` when the binary is present, advisory-skip
    // otherwise. Best-effort: a refusal (no git repo, no config) must not
    // abort the wiring the user asked for.
    let sad = wire_sad_gate(repo_root, sad_bin);

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

    // 5. session-close protocol block (DDL-9kz.1): the ritual must be
    // visible at the repo root for every harness, and gates wiring is the
    // self-owned writer that rewrites a drifted span.
    let session_protocol = crate::installer::ensure_session_protocol_block(repo_root)?;

    Ok(WiringReport {
        lefthook,
        sad,
        gitignore_added,
        beads,
        beads_exported,
        beads_staged,
        session_protocol,
    })
}

/// Wire the sad gate by invoking `sad hooks install` in `repo_root`
/// (best-effort). sad owns its managed-block markers and the gate command
/// (boundary rule); ddl only invokes the installer when the binary is
/// present — the advisory skip keeps repos without sad green.
fn wire_sad_gate(repo_root: &std::path::Path, sad_bin: Option<&std::path::Path>) -> SadGateOutcome {
    let Some(sad) = sad_bin else {
        return SadGateOutcome::NotInstalled;
    };
    let output = std::process::Command::new(sad)
        .args(["hooks", "install"])
        .current_dir(repo_root)
        .stdin(std::process::Stdio::null())
        .output();
    let note = |o: std::process::Output| {
        let mut text = String::from_utf8_lossy(&o.stdout).trim().to_string();
        let err = String::from_utf8_lossy(&o.stderr).trim().to_string();
        if text.is_empty() {
            text = err;
        } else if !err.is_empty() {
            text.push_str("; ");
            text.push_str(&err);
        }
        text
    };
    match output {
        Ok(o) if o.status.success() => SadGateOutcome::Wired(note(o)),
        Ok(o) => SadGateOutcome::Refused(note(o)),
        Err(e) => SadGateOutcome::Refused(e.to_string()),
    }
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
