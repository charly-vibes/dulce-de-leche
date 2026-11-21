//! `ddl init` install and init-cascade phases.

use super::*;

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
pub(crate) fn abort_json(json_guard: &mut Option<output::JsonCollectorGuard>) {
    if let Some(g) = json_guard.take() {
        g.abort();
    }
}

/// The installation phase: dispatches to the installer and records results.
/// Returns the init-cascade candidates (successfully installed tools; wai is
/// excluded — it is force-initialized later). Fails when the install run is
/// a total or partial failure.
#[allow(clippy::too_many_arguments)]
/// Run the installer for the selected tools and record results; returns
/// (success_count, fail_count, init-cascade candidates).
fn run_installs(
    ddl_dir: &mut DdlDir,
    platform: &crate::platform::Platform,
    selected: &[String],
    explicit_tools: bool,
    args: &crate::cli::Args,
) -> Result<(u32, u32, Vec<String>)> {
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

    // Tools whose init cascade should run: successfully installed tools
    // (wai is skipped — it is force-initialized later).
    let candidates: Vec<String> = results
        .iter()
        .filter(|r| r.success && r.tool != "wai")
        .map(|r| r.tool.to_string())
        .collect();
    Ok((success_count, fail_count, candidates))
}

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

    let (success_count, fail_count, candidates) =
        run_installs(ddl_dir, platform, selected, explicit_tools, args)?;

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
    // DDL-cxh: ah (>=0.12) refuses init without openspec/ (espectacular's
    // `ensure!(repo_root.join("openspec").exists())`). When ah is in the
    // cascade but the openspec tool-init is not (e.g. `--tools espectacular`),
    // scaffold the directory here — idempotent, never touches existing
    // content; the ordered openspec init still runs when in scope.
    if candidates.iter().any(|t| {
        crate::platform::resolve_tool(t)
            .map(|tool| tool.name == "ah")
            .unwrap_or(false)
    }) && let Err(e) = std::fs::create_dir_all("openspec")
    {
        eprintln!("  ⚠ could not scaffold openspec/ for ah init: {e}");
    }
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
