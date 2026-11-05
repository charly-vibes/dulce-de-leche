//! `ddl init` — bootstrap cascade: tool selection, installation, tool-init
//! chain, repo setup, and gates wiring.
//!
//! Decomposed from the former monolithic `cmd_init` (main.rs) into
//! behavior-preserving phases; each phase maps 1:1 onto the original code
//! blocks, with the JSON envelope guard threaded through explicitly so
//! early-error returns keep aborting the collected envelope (DDL-6zn.4).

use super::*;

/// Whether the interactive selection was cancelled by the user.
pub enum Selection {
    Cancelled,
    Tools(Vec<String>),
}

/// Names parsed from an explicit `--tools a,b,c` flag.
fn parse_tools_flag(tools: &str) -> Vec<String> {
    // Empty entries from stray commas are dropped, not resolved as tool names
    tools
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

/// Tools eligible for (re)install: not yet on PATH, or whose previous
/// attempt failed (retry semantics).
fn retryable_tools(ddl_dir: &DdlDir) -> Vec<&'static str> {
    crate::platform::MANAGED_TOOLS
        .iter()
        .filter(|t| !crate::installer::is_tool_installed(t.name) || ddl_dir.should_retry(t.name))
        .map(|t| t.name)
        .collect()
}

/// Interactive multiselect over missing/failed tools (or all on first run).
/// Returns `Cancelled` when the user declines the confirmation.
fn interactive_select(ddl_dir: &DdlDir, _args: &crate::cli::Args) -> Result<Selection> {
    let already_initialized = !ddl_dir.manifest.tools.is_empty();

    let items: Vec<(&str, &str, &str)> = if already_initialized {
        retryable_tools(ddl_dir)
            .into_iter()
            .map(|name| {
                let t = crate::platform::find_tool(name).expect("managed tool");
                (t.name, t.name, t.description)
            })
            .collect()
    } else {
        crate::platform::MANAGED_TOOLS
            .iter()
            .map(|t| (t.name, t.name, t.description))
            .collect()
    };

    if items.is_empty() {
        // Everything is installed — fall through with an empty selection
        // so the tail work (forced wai init, skill checks) still runs.
        return Ok(Selection::Tools(Vec::new()));
    }

    let selected: Vec<&str> = cliclack::multiselect("Which tools would you like to install?")
        .items(&items)
        .interact()
        .map_err(|e| DdlError::Other(format!("Selection cancelled: {e}")))?;

    let confirmed = cliclack::confirm("Proceed with installation?")
        .interact()
        .map_err(|e| DdlError::Other(format!("Confirmation cancelled: {e}")))?;

    if !confirmed {
        return Ok(Selection::Cancelled);
    }

    Ok(Selection::Tools(
        selected.into_iter().map(String::from).collect(),
    ))
}

/// Determine which tools this init run covers.
pub fn select_tools(
    tools: Option<&str>,
    args: &crate::cli::Args,
    ddl_dir: &DdlDir,
) -> Result<Selection> {
    if let Some(tools_str) = tools {
        return Ok(Selection::Tools(parse_tools_flag(tools_str)));
    }
    if args.yes || args.is_json() {
        // Non-interactive: install all tools; already-initialized runs retry
        // failed tools and skip existing ones.
        let names = if ddl_dir.manifest.tools.is_empty() {
            crate::platform::MANAGED_TOOLS
                .iter()
                .map(|t| t.name.to_string())
                .collect()
        } else {
            retryable_tools(ddl_dir)
                .into_iter()
                .map(String::from)
                .collect()
        };
        return Ok(Selection::Tools(names));
    }
    interactive_select(ddl_dir, args)
}

/// DDL-6zn.8: every explicit --tools name must resolve — a typo (or a
/// self-name like 'dulce') must fail loudly instead of vanishing silently
/// from the install list.
pub fn validate_explicit_tools(tools: Option<&str>, selected: &[String]) -> Result<()> {
    if tools.is_none() {
        return Ok(());
    }
    let errors: Vec<String> = selected
        .iter()
        .filter_map(|name| crate::platform::resolve_tool(name).err())
        .collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(DdlError::ToolNotFound(errors.join("; ")))
    }
}

/// Init candidates on the `--no-install` configure-only path: selected tools
/// (minus wai, which is force-initialized later), plus — on default full
/// re-runs — tools whose config step never succeeded (DDL-6ox).
fn no_install_candidates(ddl_dir: &DdlDir, selected: &[String], explicit: bool) -> Vec<String> {
    let mut candidates: Vec<String> = selected
        .iter()
        .filter(|t| t.as_str() != "wai")
        .cloned()
        .collect();
    if explicit {
        return candidates;
    }
    for t in crate::platform::MANAGED_TOOLS {
        let name = t.name;
        if name == "wai" || candidates.iter().any(|c| c == name) || ddl_dir.manifest.is_inited(name)
        {
            continue;
        }
        candidates.push(name.to_string());
    }
    candidates
}

/// Record install results in the manifest and print them; returns counts.
fn record_install_results(
    ddl_dir: &mut DdlDir,
    results: &[crate::installer::InstallResult],
    is_json: bool,
) -> Result<(u32, u32)> {
    let mut success_count = 0u32;
    let mut fail_count = 0u32;
    for result in results {
        if result.success {
            ddl_dir.record_installed(result.tool, &result.version, &result.method.to_string())?;
            success_count += 1;
        } else {
            ddl_dir.record_failed(result.tool, &result.method.to_string())?;
            fail_count += 1;
        }
        output::print_install_result(result.success, result.tool, &result.message, is_json);
    }
    Ok((success_count, fail_count))
}

/// Print the install summary line for non-interactive / CI mode.
fn print_install_summary(success_count: u32, fail_count: u32) {
    println!();
    if fail_count > 0 && success_count == 0 {
        println!("All {fail_count} tools failed to install.");
        println!("Check network connectivity and try again.");
    } else if fail_count > 0 {
        println!("{success_count} installed, {fail_count} failed — partial failure");
    } else {
        println!("All {success_count} tools installed successfully.");
    }
}

/// Abort the collected JSON envelope before an early error return
/// (DDL-6zn.4: the error envelope must be the only envelope on stdout).
fn abort_json(json_guard: &mut Option<output::JsonCollectorGuard>) {
    if let Some(g) = json_guard.take() {
        g.abort();
    }
}

/// The installation phase: dispatches to the installer and records results.
/// Returns the init-cascade candidates (successfully installed tools; wai is
/// excluded — it is force-initialized later). Fails when the install run is
/// a total or partial failure.
#[allow(clippy::too_many_arguments)]
pub fn install_phase(
    ddl_dir: &mut DdlDir,
    platform: &crate::platform::Platform,
    selected: &[String],
    explicit_tools: bool,
    no_install: bool,
    args: &crate::cli::Args,
    json_guard: &mut Option<output::JsonCollectorGuard>,
) -> Result<Vec<String>> {
    if no_install {
        // Configure-only mode still runs the init cascade for the selected
        // tools (DDL-6zn.3): absent tools are skipped gracefully by
        // run_tool_init's PATH/dest probe.
        output::print_success("Skipping installation (--no-install)", args.is_json());
        return Ok(no_install_candidates(ddl_dir, selected, explicit_tools));
    }
    if selected.is_empty() {
        // All tools already installed and configured
        output::print_success("All tools are already installed.", args.is_json());
        return Ok(Vec::new());
    }

    if !args.is_json() {
        let failed = ddl_dir.failed_tools();
        if !failed.is_empty() {
            println!();
            println!("Retrying {} previously failed tool(s)...", failed.len());
        }
        println!("Installing {} tools...", selected.len());
        println!();
    }

    let verbose = args.verbose_quiet.raw_count() >= 1;
    let results = if explicit_tools {
        crate::installer::install_selected_tools(selected, platform, &mut ddl_dir.manifest, verbose)
    } else {
        crate::installer::install_all_tools(platform, &mut ddl_dir.manifest, verbose)
    };

    let (success_count, fail_count) = record_install_results(ddl_dir, &results, args.is_json())?;

    // Tools whose init cascade should run: successfully installed tools.
    let candidates: Vec<String> = results
        .iter()
        .filter(|r| r.success && r.tool != "wai")
        .map(|r| r.tool.to_string())
        .collect();

    if !args.is_json() && !no_install {
        print_install_summary(success_count, fail_count);
    }

    if fail_count > 0 && success_count == 0 {
        abort_json(json_guard);
        return Err(DdlError::InstallFailed(format!(
            "All {fail_count} tool(s) failed to install"
        )));
    }
    if fail_count > 0 {
        abort_json(json_guard);
        return Err(DdlError::PartialFailure);
    }
    Ok(candidates)
}

/// Tool-init cascade (DDL-6zn.3): prerequisite-ordered (openspec init BEFORE
/// ah init), tools without init commands skipped, failures collected for the
/// resume summary instead of aborting the run.
pub fn init_cascade(
    ddl_dir: &mut DdlDir,
    candidates: &[String],
    args: &crate::cli::Args,
) -> Vec<crate::installer::InitFailure> {
    let ordered: Vec<&str> =
        crate::gates::tool_init_order(&candidates.iter().map(|s| s.as_str()).collect::<Vec<_>>());
    let non_interactive = args.yes || args.is_json();
    let verbose = args.verbose_quiet.raw_count() >= 1;
    let mut init_failures = Vec::new();
    for tool_name in ordered {
        // Idempotent-until-green (DDL-6zn.3): skip tools whose init already
        // succeeded in a previous run — they may refuse re-init (dont/bd).
        if ddl_dir.manifest.is_inited(tool_name) {
            continue;
        }
        if let Some(t) = crate::platform::find_tool(tool_name) {
            match crate::installer::run_tool_init(t, verbose, non_interactive) {
                Ok(()) => {
                    if let Err(e) = ddl_dir.record_inited(tool_name) {
                        eprintln!("  ⚠ could not record init state for {tool_name}: {e}");
                    }
                }
                Err(e) => init_failures.push(crate::installer::InitFailure {
                    tool: tool_name.to_string(),
                    detail: e.to_string(),
                }),
            }
        }
    }
    init_failures
}

/// Human-readable gates wiring report (non-JSON mode).
pub fn print_gates_report(report: &crate::gates::WiringReport) {
    use crate::gates::{BeadsChange, Change};
    let label = |c: Change| match c {
        Change::Created => "created",
        Change::Updated => "updated",
        Change::Unchanged => "already wired",
    };
    let beads_label = |c: BeadsChange| match c {
        BeadsChange::Created => "created with no-db: true",
        BeadsChange::FlippedToNoDb => "set to no-db: true",
        BeadsChange::Unchanged => "already no-db",
    };
    println!(
        "  ✓ lefthook.yml: {} (pre-commit: ah check, pretender gate, spk lint; pre-push: ah check)",
        label(report.lefthook)
    );
    if report.gitignore_added.is_empty() {
        println!("  ✓ .gitignore: tool data dirs already ignored");
    } else {
        println!(
            "  ✓ .gitignore: added {}",
            report.gitignore_added.join(", ")
        );
    }
    println!("  ✓ .beads/config.yaml: {}", beads_label(report.beads));
    if report.beads_exported {
        println!("  ✓ .beads/issues.jsonl: exported from existing DB before no-db flip");
    }
    if report.beads_staged.is_empty() {
        println!("  ✓ .beads/issues.jsonl: already tracked by git");
    } else {
        println!(
            "  ✓ .beads: staged {} (global gitignore excludes .beads/ → added with -f)",
            report.beads_staged.join(", ")
        );
        println!("  → commit the staged beads files to land the tracked export");
    }
    println!("  → run `lefthook install` to activate the hooks (core.hooksPath shim)");
}

/// JSON-mode one-line gates summary.
fn gates_json_summary(report: &crate::gates::WiringReport) -> String {
    use crate::gates::{BeadsChange, Change};
    format!(
        "gates wired: lefthook {}, gitignore +{} entries, beads {}, export {}, staged +{}",
        match report.lefthook {
            Change::Created => "created",
            Change::Updated => "updated",
            Change::Unchanged => "unchanged",
        },
        report.gitignore_added.len(),
        match report.beads {
            BeadsChange::Created => "created",
            BeadsChange::FlippedToNoDb => "flipped to no-db",
            BeadsChange::Unchanged => "unchanged",
        },
        if report.beads_exported { "yes" } else { "no" },
        report.beads_staged.len(),
    )
}

/// Gates wiring phase: lefthook managed blocks, tool-data .gitignore
/// entries, beads no-db stamping. Idempotent on re-run. File-level ops only.
pub fn gates_phase(args: &crate::cli::Args) -> Result<()> {
    let report = crate::gates::apply_gates_wiring(std::path::Path::new("."))?;
    if args.is_json() {
        output::print_success(&gates_json_summary(&report), true);
    } else {
        print_gates_report(&report);
    }
    Ok(())
}

/// Force the wai way — every `ddl init` ensures the .wai/ PARA structure
/// exists, even when wai was already installed (wai init is idempotent).
/// Init failures are advisory — they never abort the whole run.
pub fn force_wai(args: &crate::cli::Args) {
    if crate::installer::is_tool_installed("wai") {
        if let Some(wai_tool) = crate::platform::find_tool("wai") {
            let _ = crate::installer::run_tool_init(
                wai_tool,
                args.verbose_quiet.raw_count() >= 1,
                args.yes || args.is_json(),
            );
        }
    } else if !args.is_json() {
        println!("  ⚠ wai not found on PATH — run `ddl install wai` to enable the wai workflow");
    }
}

/// Resume summary for failed config steps (DDL-6ox): printed AFTER the
/// gates phase completed so the wiring is never silently skipped, with the
/// exit code staying nonzero (DDL-6zn.3 resume-summary semantics).
pub fn emit_resume_failures(
    init_failures: &[crate::installer::InitFailure],
    json_guard: &mut Option<output::JsonCollectorGuard>,
    args: &crate::cli::Args,
) -> Result<()> {
    if init_failures.is_empty() {
        return Ok(());
    }
    if !args.is_json() {
        println!();
        println!(
            "Init cascade failed for {} tool(s) — finish manually or re-run:",
            init_failures.len()
        );
        for line in crate::installer::resume_summary(init_failures) {
            println!("{line}");
        }
    } else {
        // JSON mode: buffer failure events into the collected envelope
        // and put the resume summary on stderr (stdout is machine-only).
        for f in init_failures {
            output::print_install_result(false, &f.tool, &f.detail, true);
        }
        for line in crate::installer::resume_summary(init_failures) {
            eprintln!("{line}");
        }
    }
    abort_json(json_guard);
    Err(DdlError::PartialFailure)
}

/// Entry point for `ddl init`.
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

    let selected = match select_tools(tools.as_deref(), args, &ddl_dir)? {
        Selection::Cancelled => {
            output::print_success("Installation cancelled.", args.is_json());
            return Ok(());
        }
        Selection::Tools(v) => v,
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

    ddl_dir.add_gitignore_entries(args.yes)?;

    // Canonical per-repo setup entry point (DDL-fw7). Idempotent on re-run;
    // failures are advisory, never fatal.
    crate::installer::init_repo_setup(
        &mut ddl_dir,
        args.verbose_quiet.raw_count() >= 1 && !args.is_json(),
    );

    if gates {
        gates_phase(args)?;
    }

    force_wai(args);

    // Only prompt when incitaciones is actually part of this init's scope —
    // freshly selected, already tracked in the manifest, or non-interactive
    // (which installs everything). Otherwise just hint.
    let incit_selected = selected.iter().any(|t| t == "incitaciones");
    let incit_tracked = ddl_dir.manifest.get_tool("incitaciones").is_some();
    ensure_incitaciones_skills(args, args.yes || incit_selected || incit_tracked)?;

    emit_resume_failures(&init_failures, &mut json_guard, args)?;

    if !args.is_json() {
        println!();
        println!("Done! Run `ddl status` to verify everything.");
    }

    Ok(())
}
