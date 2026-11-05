//! `ddl init` — bootstrap cascade: tool selection, installation, tool-init
//! chain, repo setup, and gates wiring.
//!
//! Decomposed from the former monolithic `cmd_init` (main.rs) into
//! behavior-preserving phases; each phase maps 1:1 onto the original code
//! blocks, with the JSON envelope guard threaded through explicitly so
//! early-error returns keep aborting the collected envelope (DDL-6zn.4).

use super::*;

mod gates;
mod phases;
mod select;

pub(crate) use gates::*;
pub(crate) use phases::*;
pub(crate) use select::*;

/// Entry point for `ddl init`.
/// Tool selection that folds the user cancellation into an early Ok.
fn select_or_cancel(
    tools: Option<&str>,
    args: &crate::cli::Args,
    ddl_dir: &DdlDir,
) -> Result<Option<Vec<String>>> {
    match select_tools(tools, args, ddl_dir)? {
        Selection::Cancelled => {
            output::print_success("Installation cancelled.", args.is_json());
            Ok(None)
        }
        Selection::Tools(v) => Ok(Some(v)),
    }
}

/// Repo setup phase: gitignore entries + canonical per-repo setup
/// (DDL-fw7). Idempotent on re-run; failures are advisory, never fatal.
fn repo_setup_phase(ddl_dir: &mut DdlDir, args: &crate::cli::Args) -> Result<()> {
    ddl_dir.add_gitignore_entries(args.yes)?;
    crate::installer::init_repo_setup(
        ddl_dir,
        args.verbose_quiet.raw_count() >= 1 && !args.is_json(),
    );
    Ok(())
}

/// Post-install tail: gates wiring, forced wai init, and the incitaciones
/// skills check (prompting only when incitaciones is in this init's scope —
/// freshly selected, already tracked, or non-interactive).
fn init_tail_phase(
    ddl_dir: &DdlDir,
    selected: &[String],
    gates: bool,
    args: &crate::cli::Args,
) -> Result<()> {
    if gates {
        gates_phase(args)?;
    }
    force_wai(args);
    let incit_selected = selected.iter().any(|t| t == "incitaciones");
    let incit_tracked = ddl_dir.manifest.get_tool("incitaciones").is_some();
    ensure_incitaciones_skills(args, args.yes || incit_selected || incit_tracked)
}

pub fn cmd_init(
    tools: Option<String>,
    no_install: bool,
    gates: bool,
    args: &crate::cli::Args,
) -> Result<()> {
    let platform = detect_platform()?;
    let mut json_guard = json_guard_or_none(args);

    output::print_banner(args.is_json());
    print_platform_banner(&platform, args);

    let mut ddl_dir = DdlDir::find_or_create()?;
    if !args.is_json() {
        println!("  ✓ .ddl/ at {}", ddl_dir.path.display());
    }

    let Some(selected) = select_or_cancel(tools.as_deref(), args, &ddl_dir)? else {
        return Ok(());
    };
    validate_explicit_tools(tools.as_deref(), &selected)?;

    let candidates = install_phase(
        &mut ddl_dir,
        &platform,
        &selected,
        tools.is_some(),
        no_install,
        args,
        &mut json_guard,
    )?;

    // DDL-6ox: config-step failures are advisory for repo setup — the run
    // continues so the gates wiring is ALWAYS written; the resume summary
    // below keeps the exit code nonzero (DDL-6zn.3).
    let init_failures = init_cascade(&mut ddl_dir, &candidates, args);

    repo_setup_phase(&mut ddl_dir, args)?;
    init_tail_phase(&ddl_dir, &selected, gates, args)?;

    emit_resume_failures(&init_failures, &mut json_guard, args)?;

    if !args.is_json() {
        println!();
        println!("Done! Run `ddl status` to verify everything.");
    }

    Ok(())
}
