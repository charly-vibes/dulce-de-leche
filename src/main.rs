//! dulce-de-leche (ddl) — CLI entry point.

use std::process;

use dulce_de_leche::dot_ddl::DdlDir;
use dulce_de_leche::error::{DdlError, Result};
use dulce_de_leche::installer::InstallMethod;
use dulce_de_leche::output;
use genesis::cli::maybe_print_version_json;
use genesis::envelope::EnvelopeKind;

fn main() {
    // Handle --version --json before clap parses.
    // Clap intercepts `--version` at the parser level, before global flags
    // like `--json` are visible, so we must check for the combo ourselves.
    if maybe_print_version_json("ddl", dulce_de_leche::VERSION) {
        return;
    }

    let args = dulce_de_leche::cli::Args::parse_or_exit();
    let want_json = args.is_json();

    if let Err(e) = run(args) {
        output::print_error(&format!("{e}"), want_json);
        process::exit(1);
    }
}

fn run(args: dulce_de_leche::cli::Args) -> Result<()> {
    use clap::CommandFactory;
    use dulce_de_leche::cli::Commands;

    match args.command {
        Some(Commands::Init {
            ref tools,
            no_install,
        }) => cmd_init(tools.clone(), no_install, &args),
        Some(Commands::Install { ref tool }) => cmd_install(tool, &args),
        Some(Commands::Status) => cmd_status(&args),
        Some(Commands::Doctor { fix }) => cmd_doctor(fix, &args),
        Some(Commands::Version { check }) => cmd_version(check, &args),
        Some(Commands::Upgrade { ref tool }) => cmd_upgrade(tool.as_deref(), &args),
        Some(Commands::Migrate { undo }) => cmd_migrate(undo, &args),
        Some(Commands::Scope) => cmd_scope(&args),
        Some(Commands::Completions { shell }) => {
            let mut cmd = dulce_de_leche::cli::Args::command();
            genesis::cli::generate_completions(&mut cmd, shell)?;
            Ok(())
        }
        None => {
            dulce_de_leche::cli::Args::command().print_help().ok();
            println!();
            Ok(())
        }
    }
}

fn cmd_init(
    tools: Option<String>,
    no_install: bool,
    args: &dulce_de_leche::cli::Args,
) -> Result<()> {
    let platform = dulce_de_leche::platform::Platform::detect()
        .ok_or_else(|| DdlError::UnsupportedPlatform("Could not detect platform".to_string()))?;

    // In JSON mode, collect all output data and emit a single envelope
    // on drop (handles all return paths including early returns).
    let _json_guard = if args.is_json() {
        Some(output::JsonCollectorGuard::start(
            dulce_de_leche::VERSION,
            genesis::envelope::EnvelopeKind::List,
        ))
    } else {
        None
    };

    output::print_banner(args.is_json());
    if !args.is_json() {
        println!("Detected platform: {} ({})", platform.os, platform.arch);
        match platform.os {
            dulce_de_leche::platform::Os::Macos => {
                if InstallMethod::Brew.check_prerequisites().is_ok() {
                    println!("Available package manager: brew");
                } else {
                    println!("Available package manager: cargo / binary download");
                }
            }
            dulce_de_leche::platform::Os::Linux => {
                println!("Available package manager: cargo / binary download");
            }
            dulce_de_leche::platform::Os::Windows => {
                if InstallMethod::Scoop.check_prerequisites().is_ok() {
                    println!("Available package manager: scoop");
                } else {
                    println!("Available package manager: binary download");
                }
            }
        }
    }

    let mut ddl_dir = DdlDir::find_or_create()?;
    if !args.is_json() {
        println!("  ✓ .ddl/ at {}", ddl_dir.path.display());
    }

    // Determine which tools to install
    let selected_tools: Vec<String> = if let Some(ref tools_str) = tools {
        // Explicit list from --tools flag
        tools_str.split(',').map(|s| s.trim().to_string()).collect()
    } else if args.yes || args.is_json() {
        // Non-interactive: install all tools
        // Already initialized: retry failed tools, skip existing
        if ddl_dir.manifest.tools.is_empty() {
            dulce_de_leche::platform::MANAGED_TOOLS
                .iter()
                .map(|t| t.name.to_string())
                .collect()
        } else {
            // Only install tools that are not yet installed or previously failed
            dulce_de_leche::platform::MANAGED_TOOLS
                .iter()
                .filter(|t| {
                    !dulce_de_leche::installer::is_tool_installed(t.name)
                        || ddl_dir.should_retry(t.name)
                })
                .map(|t| t.name.to_string())
                .collect()
        }
    } else {
        // Interactive: prompt user to select tools
        let already_initialized = !ddl_dir.manifest.tools.is_empty();

        let items: Vec<(&str, &str, &str)> = if already_initialized {
            // Show only missing/failed tools
            dulce_de_leche::platform::MANAGED_TOOLS
                .iter()
                .filter(|t| {
                    !dulce_de_leche::installer::is_tool_installed(t.name)
                        || ddl_dir.should_retry(t.name)
                })
                .map(|t| (t.name, t.name, t.description))
                .collect()
        } else {
            dulce_de_leche::platform::MANAGED_TOOLS
                .iter()
                .map(|t| (t.name, t.name, t.description))
                .collect()
        };

        if items.is_empty() {
            output::print_success(
                "All tools are already installed and configured.",
                args.is_json(),
            );
            return Ok(());
        }

        let selected: Vec<&str> = cliclack::multiselect("Which tools would you like to install?")
            .items(&items)
            .interact()
            .map_err(|e| DdlError::Other(format!("Selection cancelled: {e}")))?;

        let confirmed = cliclack::confirm("Proceed with installation?")
            .interact()
            .map_err(|e| DdlError::Other(format!("Confirmation cancelled: {e}")))?;

        if !confirmed {
            output::print_success("Installation cancelled.", args.is_json());
            return Ok(());
        }

        selected.into_iter().map(|s| s.to_string()).collect()
    };

    if no_install {
        output::print_success("Skipping installation (--no-install)", args.is_json());
    } else if selected_tools.is_empty() {
        // All tools already installed and configured
        output::print_success("All tools are already installed.", args.is_json());
    } else {
        if !args.is_json() {
            let failed = ddl_dir.failed_tools();
            if !failed.is_empty() {
                println!();
                println!("Retrying {} previously failed tool(s)...", failed.len());
            }
            println!("Installing {} tools...", selected_tools.len());
            println!();
        }

        let results = if tools.is_some() {
            dulce_de_leche::installer::install_selected_tools(
                &selected_tools,
                &platform,
                &mut ddl_dir.manifest,
                args.verbose_quiet.raw_count() >= 1,
            )
        } else {
            dulce_de_leche::installer::install_all_tools(
                &platform,
                &mut ddl_dir.manifest,
                args.verbose_quiet.raw_count() >= 1,
            )
        };

        let mut success_count = 0u32;
        let mut fail_count = 0u32;

        for result in &results {
            if result.success {
                ddl_dir.record_installed(
                    result.tool,
                    &result.version,
                    &result.method.to_string(),
                )?;
                success_count += 1;
            } else {
                ddl_dir.record_failed(result.tool, &result.method.to_string())?;
                fail_count += 1;
            }
            output::print_install_result(
                result.success,
                result.tool,
                &result.message,
                args.is_json(),
            );
        }

        // Run init commands for successfully installed tools
        for result in &results {
            if result.success {
                let tool = dulce_de_leche::platform::find_tool(result.tool);
                if let Some(t) = tool {
                    let _ = dulce_de_leche::installer::run_tool_init(
                        t,
                        args.verbose_quiet.raw_count() >= 1,
                    );
                }
            }
        }

        // Summary and error reporting for non-interactive / CI mode
        if !args.is_json() && !no_install {
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

        // Return error if all tools failed
        if fail_count > 0 && success_count == 0 {
            return Err(DdlError::InstallFailed(format!(
                "All {fail_count} tool(s) failed to install"
            )));
        }

        // Return PartialFailure if some tools failed
        if fail_count > 0 {
            return Err(DdlError::PartialFailure);
        }
    }

    ddl_dir.add_gitignore_entries(args.yes)?;

    if !args.is_json() {
        println!();
        println!("Done! Run `ddl status` to verify everything.");
    }

    Ok(())
}

fn cmd_install(tool_name: &str, args: &dulce_de_leche::cli::Args) -> Result<()> {
    let platform = dulce_de_leche::platform::Platform::detect()
        .ok_or_else(|| DdlError::UnsupportedPlatform("Could not detect platform".to_string()))?;

    let tool = dulce_de_leche::platform::find_tool(tool_name).ok_or_else(|| {
        let names: Vec<&str> = dulce_de_leche::platform::MANAGED_TOOLS
            .iter()
            .map(|t| t.name)
            .collect();
        let suggestion = dulce_de_leche::platform::did_you_mean(tool_name, &names);
        DdlError::ToolNotFound(if let Some(s) = suggestion {
            format!("Unknown tool '{tool_name}'. Did you mean '{s}'?")
        } else {
            format!("Unknown tool '{tool_name}'")
        })
    })?;

    let mut ddl_dir = DdlDir::find_or_create()?;

    if dulce_de_leche::installer::is_tool_installed(tool.name) {
        let msg = if let Some(entry) = ddl_dir.manifest.get_tool(tool.name) {
            format!(
                "{} v{} is already installed (via {})",
                tool.name, entry.installed, entry.source
            )
        } else {
            format!("{} is already installed on PATH", tool.name)
        };
        output::print_success(&msg, args.is_json());
        return Ok(());
    }

    if !args.is_json() {
        println!("Installing {}...", tool.name);
    }
    let result = dulce_de_leche::installer::install_tool(
        tool,
        &platform,
        &mut ddl_dir.manifest,
        args.verbose_quiet.raw_count() >= 1,
    );

    if result.success {
        ddl_dir.record_installed(result.tool, &result.version, &result.method.to_string())?;
        output::print_install_result(true, result.tool, &result.message, args.is_json());
        let _ = dulce_de_leche::installer::run_tool_init(tool, args.verbose_quiet.raw_count() >= 1);
        Ok(())
    } else {
        ddl_dir.record_failed(result.tool, &result.method.to_string())?;
        Err(DdlError::InstallFailed(result.message))
    }
}

fn cmd_status(args: &dulce_de_leche::cli::Args) -> Result<()> {
    use dulce_de_leche::diagnostics;

    let ddl_dir = DdlDir::find();
    let report = diagnostics::build_status_report(ddl_dir.as_ref());

    if args.is_json() {
        let data = serde_json::json!(report);
        let json_str = output::json_output(dulce_de_leche::VERSION, true, EnvelopeKind::List, data, vec![], vec![])?;
        println!("{json_str}");
        return Ok(());
    }

    println!("dulce-de-leche — ecosystem status");
    println!();

    for section in &report.sections {
        for item in &section.items {
            println!("{}", diagnostics::format_health_line(item));
        }
    }

    println!();
    println!("{}", diagnostics::status_summary(&report.sections));

    Ok(())
}

fn cmd_doctor(fix: bool, args: &dulce_de_leche::cli::Args) -> Result<()> {
    use dulce_de_leche::diagnostics;

    let ddl_dir = DdlDir::find();

    if args.is_json() {
        let messages = diagnostics::run_full_diagnostic(ddl_dir.as_ref(), fix)?;
        let data = serde_json::json!({ "diagnostics": messages });
        let json_str = output::json_output(dulce_de_leche::VERSION, true, EnvelopeKind::Doctor, data, vec![], vec![])?;
        println!("{json_str}");
        return Ok(());
    }

    println!("dulce-de-leche — diagnostics");
    println!();

    let messages = diagnostics::run_full_diagnostic(ddl_dir.as_ref(), fix)?;
    for msg in &messages {
        println!("  {}", msg);
    }

    Ok(())
}

fn cmd_version(check: bool, args: &dulce_de_leche::cli::Args) -> Result<()> {
    let ddl_version = dulce_de_leche::VERSION;

    if args.is_json() {
        let mut tools = Vec::new();
        let latest_versions = if check {
            dulce_de_leche::installer::check_all_latest_versions()
        } else {
            std::collections::HashMap::new()
        };

        if let Some(ddl_dir) = DdlDir::find() {
            let matrix = dulce_de_leche::compat::load_compatibility(&ddl_dir.path);
            let mut names: Vec<&String> = ddl_dir.manifest.tools.keys().collect();
            names.sort();
            for name in names {
                if let Some(entry) = ddl_dir.manifest.tools.get(name) {
                    let constraint = matrix.constraint(name).unwrap_or("*");
                    let compatible = matrix.is_compatible(name, &entry.installed);
                    let latest = latest_versions.get(name.as_str());
                    let update_available = latest.is_some_and(|lv| *lv != entry.installed);
                    tools.push(serde_json::json!({
                        "name": name,
                        "version": entry.installed,
                        "latest": latest,
                        "update_available": update_available,
                        "source": entry.source,
                        "status": entry.status,
                        "compatible": compatible,
                        "constraint": constraint
                    }));
                }
            }
        }
        let data = serde_json::json!({
            "ddl_version": ddl_version,
            "check": check,
            "tools": tools
        });
        let json_str = output::json_output(dulce_de_leche::VERSION, true, EnvelopeKind::Version, data, vec![], vec![])?;
        println!("{json_str}");

        if check && tools.iter().any(|t| t["update_available"] == true) {
            return Err(DdlError::PartialFailure);
        }
        return Ok(());
    }

    println!("ddl: {}", ddl_version);
    println!();

    match DdlDir::find() {
        Some(ddl_dir) => {
            let mut names: Vec<&String> = ddl_dir.manifest.tools.keys().collect();
            names.sort();
            for name in names {
                if let Some(entry) = ddl_dir.manifest.tools.get(name) {
                    println!("  {:12} v{} (via {})", name, entry.installed, entry.source);
                }
            }
        }
        None => {
            use dulce_de_leche::diagnostics;
            for tool in dulce_de_leche::platform::MANAGED_TOOLS {
                if diagnostics::which(tool.name).is_some() {
                    let version =
                        diagnostics::get_tool_version(tool).unwrap_or_else(|| "?".to_string());
                    println!("  {:12} v{}", tool.name, version);
                }
            }
        }
    }

    if check {
        println!();
        println!("Checking for updates... (requires network)");
        println!();

        let latest_versions = dulce_de_leche::installer::check_all_latest_versions();
        let network_ok = !latest_versions.is_empty();

        if !network_ok {
            println!("  ⚠ Network unavailable — showing cached/embedded versions");
            println!();
        }

        let mut any_outdated = false;
        let matrix = if let Some(ddl_dir) = DdlDir::find() {
            dulce_de_leche::compat::load_compatibility(&ddl_dir.path)
        } else {
            dulce_de_leche::compat::CompatibilityMatrix::embedded()
        };

        for tool in dulce_de_leche::platform::MANAGED_TOOLS {
            let installed = dulce_de_leche::installer::is_tool_installed(tool.name);
            let constraint = matrix.constraint(tool.name).unwrap_or("*");
            let version = if installed {
                dulce_de_leche::installer::get_installed_version(tool.name)
                    .unwrap_or_else(|| "?".to_string())
            } else {
                "not installed".to_string()
            };

            let latest = latest_versions.get(tool.name);
            let status = if !installed {
                "".to_string()
            } else if let Some(lv) = latest {
                if *lv != version {
                    any_outdated = true;
                    format!("update available: v{lv}")
                } else {
                    "up to date".to_string()
                }
            } else if network_ok {
                // Network worked but no version found for this tool
                "".to_string()
            } else {
                "".to_string()
            };

            if installed {
                println!(
                    "  {:12} v{} (constraint: {}) {}",
                    tool.name, version, constraint, status
                );
            } else {
                println!(
                    "  {:12} {} (constraint: {})",
                    tool.name, version, constraint
                );
            }
        }

        if any_outdated {
            println!();
            println!("Some tools have updates available. Run `ddl upgrade` to update.");
            return Err(DdlError::PartialFailure);
        }
    }

    Ok(())
}

fn cmd_upgrade(tool_name: Option<&str>, args: &dulce_de_leche::cli::Args) -> Result<()> {
    let platform = dulce_de_leche::platform::Platform::detect()
        .ok_or_else(|| DdlError::UnsupportedPlatform("Could not detect platform".to_string()))?;

    let mut ddl_dir = DdlDir::find_or_create()?;

    if let Some(name) = tool_name {
        let tool = dulce_de_leche::platform::find_tool(name)
            .ok_or_else(|| DdlError::ToolNotFound(name.to_string()))?;
        if !args.is_json() {
            println!("Upgrading {}...", tool.name);
        }
        let result = dulce_de_leche::installer::upgrade_tool(
            tool,
            &platform,
            &mut ddl_dir.manifest,
            args.verbose_quiet.raw_count() >= 1,
        );
        ddl_dir.save_manifest()?;
        output::print_install_result(result.success, result.tool, &result.message, args.is_json());
        if !result.success {
            return Err(DdlError::InstallFailed(result.message));
        }
    } else {
        if !args.is_json() {
            println!("Upgrading all tools...");
        }
        let results = dulce_de_leche::installer::upgrade_all_tools(
            &platform,
            &mut ddl_dir.manifest,
            args.verbose_quiet.raw_count() >= 1,
        );
        ddl_dir.save_manifest()?;

        let mut success_count = 0u32;
        let mut fail_count = 0u32;
        let mut skipped_count = 0u32;

        for result in &results {
            match result.method {
                dulce_de_leche::installer::InstallMethod::Skipped => {
                    skipped_count += 1;
                }
                _ if result.success => {
                    success_count += 1;
                }
                _ => {
                    fail_count += 1;
                }
            }
            output::print_install_result(
                result.success,
                result.tool,
                &result.message,
                args.is_json(),
            );
        }

        if !args.is_json() {
            println!();
            if fail_count > 0 && success_count == 0 && skipped_count == 0 {
                println!("All upgrades failed. Check network connectivity.");
            } else if fail_count > 0 {
                println!(
                    "{success_count} upgraded, {fail_count} failed, {skipped_count} up to date"
                );
            } else if skipped_count > 0 {
                println!("All {skipped_count} tools are already up to date.");
            } else {
                println!("All {success_count} tools upgraded successfully.");
            }
        }

        if fail_count > 0 && success_count == 0 && skipped_count == 0 {
            return Err(DdlError::InstallFailed("All upgrades failed".to_string()));
        }
        if fail_count > 0 {
            return Err(DdlError::PartialFailure);
        }
    }

    Ok(())
}

fn cmd_migrate(undo: bool, args: &dulce_de_leche::cli::Args) -> Result<()> {
    let ddl_dir = DdlDir::find_or_create()?;

    if undo {
        let migrated = dulce_de_leche::dot_ddl::migrated_tools(&ddl_dir);
        if migrated.is_empty() {
            output::print_success("No migrated configs found.", args.is_json());
            return Ok(());
        }
        for (tool_name, legacy_path) in &migrated {
            if !args.is_json() {
                println!("  Restoring {}...", tool_name);
            }
            ddl_dir.unmigrate_tool(tool_name, legacy_path)?;
            output::print_install_result(
                true,
                tool_name,
                &format!("{} restored to {}", tool_name, legacy_path.display()),
                args.is_json(),
            );
        }
        return Ok(());
    }

    let legacy_configs = ddl_dir.detect_legacy_configs();
    if legacy_configs.is_empty() {
        output::print_success(
            "No legacy configs found. Nothing to migrate.",
            args.is_json(),
        );
        return Ok(());
    }

    if !args.is_json() {
        println!("Phase 1 migration — moving configs under .ddl/...");
    }

    for (tool_name, legacy_path) in &legacy_configs {
        if !args.is_json() {
            println!(
                "  Migrating {} from {}...",
                tool_name,
                legacy_path.display()
            );
        }
        ddl_dir.migrate_tool(tool_name, legacy_path)?;
        output::print_install_result(
            true,
            tool_name,
            &format!(
                "{} migrated to {}",
                tool_name,
                ddl_dir.tool_path(tool_name).display()
            ),
            args.is_json(),
        );
    }

    let mut ddl_dir = ddl_dir;
    ddl_dir.manifest.migration_state = "phase1".to_string();
    ddl_dir.save_manifest()?;

    if !args.is_json() {
        println!();
        println!("Done. All configs are now under .ddl/.");
        println!("Run `ddl migrate --undo` to restore the previous layout.");
    }

    Ok(())
}

fn cmd_scope(args: &dulce_de_leche::cli::Args) -> Result<()> {
    if let Some(ddl_dir) = dulce_de_leche::find_ddl_dir() {
        if args.is_json() {
            let data = serde_json::json!({"ddl_dir": ddl_dir.display().to_string()});
            let json_str = output::json_output(dulce_de_leche::VERSION, true, EnvelopeKind::Info, data, vec![], vec![])?;
            println!("{json_str}");
        } else {
            println!("{}", ddl_dir.display());
        }
    } else if args.is_json() {
        let data = serde_json::json!({"ddl_dir": null});
        let json_str = output::json_output(dulce_de_leche::VERSION, true, EnvelopeKind::Info, data, vec![], vec![])?;
        println!("{json_str}");
    } else {
        println!("No .ddl/ directory found (walked up from CWD)");
    }
    Ok(())
}
