//! Tool init commands, PATH wiring, and install reconciliation.

use super::*;
use crate::dot_ddl::DdlDir;
use crate::error::{DdlError, Result};
use crate::platform::{MANAGED_TOOLS, Os, Platform, Tool};
use std::path::{Path, PathBuf};
use std::process::Command;

/// A tool-init step that failed during the cascade.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitFailure {
    pub tool: String,
    pub detail: String,
}

/// The init command for a tool, if it has one.
///
/// `non_interactive` selects prompt-free variants: wizards (pretender) would
/// otherwise leak interactive prompt text into `--yes` output, and config
/// prompts (openspec, bd) would hang CI.
///
/// Tools without an init command (fotos-mcp, incitaciones, turu) return None
/// — ddl must skip them silently, never invoke a bogus `init` subcommand.
pub fn init_command(
    tool_name: &str,
    non_interactive: bool,
) -> Option<(&'static str, Vec<&'static str>)> {
    let (cmd, args): (&'static str, &[&'static str]) = match tool_name {
        "wai" => ("wai", &["init"]),
        // `dont init` first: `prime` fails when no .dont/ project exists yet.
        "dont" => ("dont", &["init"]),
        "ah" => ("ah", &["init"]),
        "pretender" if non_interactive => ("pretender", &["init", "--non-interactive"]),
        "pretender" => ("pretender", &["init"]),
        "testaruda" => ("testaruda", &["init"]),
        "vampiro" => ("vampiro", &["init"]),
        // bd init is interactive by default; --non-interactive skips prompts
        // (auto-skips Claude hooks when stdout is not a TTY).
        "bd" => ("bd", &["init", "--non-interactive"]),
        // openspec init scaffolds openspec/ and prompts for AI-tool config;
        // --tools none skips that prompt.
        "openspec" => ("openspec", &["init", "--tools", "none"]),
        "specodelic" => ("specodelic", &["doctor"]),
        _ => return None,
    };
    Some((cmd, args.to_vec()))
}

/// `dir` prepended to an inherited PATH (so freshly downloaded binaries in
/// `~/.ddl/bin` can be initialized and find their own siblings).
pub fn prepend_path(
    dir: &std::path::Path,
    existing: Option<&std::ffi::OsStr>,
) -> std::ffi::OsString {
    let mut paths = vec![dir.to_path_buf()];
    if let Some(rest) = existing
        .filter(|p| !p.is_empty())
        .map(std::env::split_paths)
    {
        paths.extend(rest);
    }
    std::env::join_paths(paths).unwrap_or_else(|_| dir.as_os_str().to_os_string())
}

/// Human-readable resume lines for failed init steps: what failed and the
/// exact command(s) to finish manually (or via a selective re-run).
pub fn resume_summary(failures: &[InitFailure]) -> Vec<String> {
    failures
        .iter()
        .map(|f| {
            let (cmd, args) =
                init_command(&f.tool, false).unwrap_or((f.tool.as_str(), Vec::new()));
            let manual = args.iter().fold(cmd.to_string(), |acc, a| format!("{acc} {a}"));
            format!(
                "  ✗ {} init failed: {}\n      finish with: {}\n      or re-run: ddl init --tools {}",
                f.tool, f.detail, manual, f.tool
            )
        })
        .collect()
}

// ── Init-side repo setup (DDL-fw7) ────────────────────────────────────

/// Make `ddl init` the canonical per-repo setup entry point.
///
/// Three idempotent pieces the doctor used to only *hint* at (DDL-6zn.5):
///
/// 1. write ddl's own managed block into AGENTS.md (`agents_block()`,
///    genesis::aix provenance-aware format) — agents_hand-wiring what init
///    should wire was the dominant setup friction;
/// 2. reconcile pre-existing tool installs into `.ddl/manifest.json` —
///    doctor flagged "not tracked in manifest" with no fix;
/// 3. scaffold `versions.ddl.toml` from the fleet pin
///    ([`crate::manifest::FLEET_VERSIONS_PIN`]) when absent — doctor
///    hinted "CI installs are unpinned" but never created it.
///
/// The repo root is the enclosing git root (the `ddl.repo.*` doctor checks
/// key off the same boundary); a plain directory without git falls back to
/// the cwd, matching `DdlDir::find_or_create`. Failures are reported but
/// never abort the whole init: they are advisory setup gaps, not install
/// failures.
pub fn init_repo_setup(ddl_dir: &mut DdlDir, verbose: bool) {
    let repo_root = repo_root();

    match ensure_agents_block(&repo_root) {
        Ok(true) => eprintln!("  ✓ AGENTS.md: DDL managed block written"),
        Ok(false) => verbose_print(verbose, "AGENTS.md: DDL managed block already present"),
        Err(e) => eprintln!("  ⚠ could not write DDL managed block to AGENTS.md: {e}"),
    }

    match scaffold_versions_pin(&repo_root) {
        Ok(true) => eprintln!("  ✓ versions.ddl.toml: scaffolded from the fleet pin"),
        Ok(false) => verbose_print(verbose, "versions.ddl.toml already present"),
        Err(e) => eprintln!("  ⚠ could not scaffold versions.ddl.toml: {e}"),
    }

    match reconcile_existing_installs(ddl_dir) {
        Ok(reconciled) if reconciled.is_empty() => {
            verbose_print(verbose, "manifest: no pre-existing installs to reconcile")
        }
        Ok(reconciled) => eprintln!(
            "  ✓ manifest: reconciled pre-existing installs ({})",
            reconciled.join(", ")
        ),
        Err(e) => eprintln!("  ⚠ could not reconcile manifest: {e}"),
    }
}

/// The repo root for init's repo-scoped setup files: the enclosing git
/// root, or the cwd when there is none (same fallback semantics as
/// `DdlDir::find_or_create`).
pub(crate) fn repo_root() -> PathBuf {
    crate::diagnostics::find_git_root(Path::new(".")).unwrap_or_else(|| PathBuf::from("."))
}

/// Write ddl's own managed block into AGENTS.md at `repo_root` if absent.
/// Returns `Ok(false)` when the block is already present (idempotent);
/// creates the file when missing. Returns `Ok(true)` when written.
pub fn ensure_agents_block(repo_root: &Path) -> Result<bool> {
    let path = repo_root.join("AGENTS.md");
    let existing = path
        .exists()
        .then(|| std::fs::read_to_string(&path))
        .transpose()
        .map_err(DdlError::Io)?;
    if existing
        .as_deref()
        .is_some_and(|text| text.contains(crate::DDL_AGENTS_MARKER))
    {
        return Ok(false);
    }
    let block = crate::agents_block();
    let mut text = existing.unwrap_or_default();
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str(&block);
    text.push('\n');
    std::fs::write(&path, text).map_err(DdlError::Io)?;
    Ok(true)
}

/// Scaffold `versions.ddl.toml` at `repo_root` from the fleet pin when the
/// repo has none. Returns `Ok(false)` when a pins file already exists —
/// a repo's committed pins are its own source of truth and are never
/// overwritten by init.
pub fn scaffold_versions_pin(repo_root: &Path) -> Result<bool> {
    let path = repo_root.join("versions.ddl.toml");
    if path.exists() {
        return Ok(false);
    }
    std::fs::write(&path, crate::manifest::FLEET_VERSIONS_PIN).map_err(DdlError::Io)?;
    Ok(true)
}

/// Reconcile pre-existing tool installs into the per-repo manifest.
///
/// For every managed tool that is installed (on PATH) but not tracked in
/// `.ddl/manifest.json`, record an `installed` entry with the probed
/// version and a reconciled source. Returns the reconciled tool names.
/// This is inventory bookkeeping only — it never touches the install or
/// verification protocol (DDL-yts/5bs/uu8 territory).
pub fn reconcile_existing_installs(ddl_dir: &mut DdlDir) -> Result<Vec<String>> {
    let mut reconciled = Vec::new();
    for tool in MANAGED_TOOLS {
        if !is_tool_installed(tool.name) || ddl_dir.manifest.is_installed(tool.name) {
            continue;
        }
        let version = get_installed_version(tool.name).unwrap_or_else(|| "unknown".to_string());
        ddl_dir.record_installed(
            tool.name,
            &version,
            "pre-existing install (reconciled by ddl init)",
        )?;
        reconciled.push(tool.name.to_string());
    }
    Ok(reconciled)
}

/// Run a tool's init command after installation.
pub fn run_tool_init(tool: &Tool, verbose: bool, non_interactive: bool) -> Result<()> {
    let Some((cmd, args)) = init_command(tool.name, non_interactive) else {
        return Ok(()); // tool has no init command — skip silently
    };

    // Check if tool is on PATH; if not, probe its .ddl/bin destination
    // directly (DDL-6zn.3: freshly downloaded binaries are not on PATH yet —
    // the old code skipped their init with only a hint).
    let on_path = is_tool_installed(cmd);
    let at_dest = Platform::detect().map(|p| binary_dest_path(tool, &p));
    let at_dest_exists = at_dest.as_ref().is_some_and(|p| p.is_file());
    if !on_path && !at_dest_exists {
        eprintln!("  ⚠ {} not found on PATH — skipping init", cmd);
        return Ok(());
    }

    verbose_print(verbose, &format!("running: {} init", cmd));

    let mut command = if on_path {
        Command::new(cmd)
    } else {
        Command::new(at_dest.expect("checked at_dest_exists above"))
    };
    command.args(&args);
    // DDL-uu8: restricted child environment; prepend the ddl bin dir so
    // downloaded tools see their siblings (override wins over the allowlist).
    let mut overrides: Vec<(&str, std::ffi::OsString)> = Vec::new();
    if let Some(bin_dir) = ddl_bin_dir() {
        overrides.push((
            "PATH",
            prepend_path(&bin_dir, std::env::var_os("PATH").as_deref()),
        ));
    }
    apply_child_policy(&mut command, ExecutionScope::ToolInit, &overrides).map_err(|e| {
        DdlError::InstallFailed(format!("{} init rejected by child policy: {e}", tool.name))
    })?;

    // DDL-1z1: the init child runs under the runtime and captured-output
    // budgets — a hung or flooding init child is killed (with its process
    // group) and the budget is named in the failure, which feeds the resume
    // summary and JSON diagnostics.
    let output = run_child_captured(&mut command, init_deadline_for(tool.name))?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let first_line = stdout.lines().next().unwrap_or("");
        if !first_line.is_empty() {
            eprintln!("  ✓ {} initialized: {}", tool.name, first_line);
        } else {
            eprintln!("  ✓ {} initialized", tool.name);
        }
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!(
            "  ✗ {} init exited with code {}: {}",
            tool.name,
            output.status.code().unwrap_or(-1),
            stderr.lines().next().unwrap_or("unknown error")
        );
        // DDL-6zn.3: init failures are no longer advisory warnings — the
        // cascade collects them and the run exits with a resume summary,
        // instead of silently leaving the repo half-configured.
        Err(DdlError::InstallFailed(format!(
            "{} init exited with code {}: {}",
            tool.name,
            output.status.code().unwrap_or(-1),
            stderr.lines().next().unwrap_or("unknown error")
        )))
    }
}

pub(crate) fn ensure_on_path(dir: &Path, platform: &Platform) -> Result<()> {
    let dir_str = dir.to_string_lossy().to_string();

    if let Some(paths) = std::env::var_os("PATH") {
        for path in std::env::split_paths(&paths) {
            if path == dir {
                return Ok(());
            }
        }
    }

    match platform.os {
        Os::Windows => {
            eprintln!("  ⚠ Add {} to your PATH environment variable", dir_str);
            eprintln!("     set PATH=%PATH%;{}", dir_str);
        }
        _ => {
            let rc_file = if dirs::home_dir().is_some_and(|h| h.join(".zshrc").exists()) {
                "~/.zshrc"
            } else if dirs::home_dir().is_some_and(|h| h.join(".bashrc").exists()) {
                "~/.bashrc"
            } else {
                "your shell config"
            };
            eprintln!("  ⚠ Add {} to your PATH", dir_str);
            eprintln!("     export PATH=\"{}:$PATH\"", dir_str);
            eprintln!("     Add the above to {rc_file}");
        }
    }
    Ok(())
}
