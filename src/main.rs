//! dulce-de-leche (ddl) — CLI entry point.

use clap::CommandFactory;
use std::process;

use dulce_de_leche::dot_ddl::DdlDir;
use dulce_de_leche::error::{DdlError, Result};
use dulce_de_leche::installer::InstallMethod;

fn main() {
    let args = dulce_de_leche::cli::Args::parse_or_exit();

    if let Err(e) = run(args) {
        eprintln!("{e:?}");
        process::exit(1);
    }
}

fn run(args: dulce_de_leche::cli::Args) -> Result<()> {
    use dulce_de_leche::cli::Commands;

    match args.command {
        Some(Commands::Init { ref tools, no_install }) => {
            cmd_init(tools.clone(), no_install, &args)
        }
        Some(Commands::Install { ref tool }) => cmd_install(tool, &args),
        Some(Commands::Status) => cmd_status(&args),
        Some(Commands::Doctor { fix }) => cmd_doctor(fix, &args),
        Some(Commands::Version { check }) => cmd_version(check, &args),
        Some(Commands::Upgrade { ref tool }) => cmd_upgrade(tool.as_deref(), &args),
        Some(Commands::Migrate { undo }) => cmd_migrate(undo, &args),
        Some(Commands::Scope) => cmd_scope(&args),
        None => {
            dulce_de_leche::cli::Args::command().print_help().ok();
            println!();
            Ok(())
        }
    }
}

fn cmd_init(tools: Option<String>, no_install: bool, args: &dulce_de_leche::cli::Args) -> Result<()> {
    let platform = dulce_de_leche::platform::Platform::detect()
        .ok_or_else(|| DdlError::UnsupportedPlatform("Could not detect platform".to_string()))?;

    println!("╭──────────────────────────────────────╮");
    println!("│  dulce-de-leche — charly-vibes       │");
    println!("│  bundle orchestrator v{}       │", dulce_de_leche::VERSION);
    println!("╰──────────────────────────────────────╯");
    println!();
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

    let mut ddl_dir = DdlDir::find_or_create()?;
    println!("  ✓ .ddl/ at {}", ddl_dir.path.display());

    let selected_tools: Vec<String> = if let Some(ref tools_str) = tools {
        tools_str.split(',').map(|s| s.trim().to_string()).collect()
    } else {
        dulce_de_leche::platform::MANAGED_TOOLS
            .iter()
            .map(|t| t.name.to_string())
            .collect()
    };

    if !no_install {
        let failed = ddl_dir.failed_tools();
        if !failed.is_empty() {
            println!();
            println!("Retrying {} previously failed tool(s)...", failed.len());
        }

        println!("Installing {} tools...", selected_tools.len());
        println!();

        let results = if tools.is_some() {
            dulce_de_leche::installer::install_selected_tools(
                &selected_tools, &platform, &mut ddl_dir.manifest, args.verbose,
            )
        } else {
            dulce_de_leche::installer::install_all_tools(
                &platform, &mut ddl_dir.manifest, args.verbose,
            )
        };

        for result in &results {
            if result.success {
                ddl_dir.record_installed(result.tool, "unknown", &result.method.to_string())?;
            } else {
                ddl_dir.record_failed(result.tool, &result.method.to_string())?;
            }
        }

        println!();
        for result in &results {
            let icon = if result.success { "✓" } else { "✗" };
            println!("  {} {}", icon, result.message);
        }

        println!();
        for result in &results {
            if result.success {
                let tool = dulce_de_leche::platform::find_tool(result.tool);
                if let Some(t) = tool {
                    let _ = dulce_de_leche::installer::run_tool_init(t, args.verbose);
                }
            }
        }
    } else {
        println!("Skipping installation (--no-install)");
    }

    ddl_dir.add_gitignore_entries(args.yes)?;

    println!();
    println!("Done! Run `ddl status` to verify everything.");
    Ok(())
}

fn cmd_install(tool_name: &str, args: &dulce_de_leche::cli::Args) -> Result<()> {
    let platform = dulce_de_leche::platform::Platform::detect()
        .ok_or_else(|| DdlError::UnsupportedPlatform("Could not detect platform".to_string()))?;

    let tool = dulce_de_leche::platform::find_tool(tool_name).ok_or_else(|| {
        let names: Vec<&str> =
            dulce_de_leche::platform::MANAGED_TOOLS.iter().map(|t| t.name).collect();
        let suggestion = dulce_de_leche::platform::did_you_mean(tool_name, &names);
        DdlError::ToolNotFound(if let Some(s) = suggestion {
            format!("Unknown tool '{tool_name}'. Did you mean '{s}'?")
        } else {
            format!("Unknown tool '{tool_name}'")
        })
    })?;

    let mut ddl_dir = DdlDir::find_or_create()?;

    if dulce_de_leche::installer::is_tool_installed(tool.name) {
        println!("{} is already installed", tool.name);
        if let Some(entry) = ddl_dir.manifest.get_tool(tool.name) {
            println!("  Version: {} (source: {})", entry.installed, entry.source);
        }
        println!("  Run `ddl upgrade {}` to update", tool.name);
        return Ok(());
    }

    println!("Installing {}...", tool.name);
    let result = dulce_de_leche::installer::install_tool(tool, &platform, &mut ddl_dir.manifest, args.verbose);

    if result.success {
        ddl_dir.record_installed(result.tool, "unknown", &result.method.to_string())?;
        println!("  ✓ {}", result.message);
        let _ = dulce_de_leche::installer::run_tool_init(tool, args.verbose);
        Ok(())
    } else {
        ddl_dir.record_failed(result.tool, &result.method.to_string())?;
        Err(DdlError::InstallFailed(result.message))
    }
}

fn cmd_status(args: &dulce_de_leche::cli::Args) -> Result<()> {
    use dulce_de_leche::diagnostics;

    let ddl_dir = DdlDir::find_or_create().ok();
    let health = diagnostics::collect_all_health(ddl_dir.as_ref());

    if args.json {
        let tools: Vec<serde_json::Value> = health.iter().map(|h| {
            serde_json::json!({
                "name": h.name,
                "description": h.description,
                "installed": h.installed,
                "version": h.version,
                "config_ok": h.config_ok,
                "suggestion": h.suggestion
            })
        }).collect();

        let json = serde_json::json!({
            "ok": true,
            "version": dulce_de_leche::VERSION,
            "data": { "tools": tools },
            "warnings": [],
            "hints": []
        });
        println!("{}", serde_json::to_string_pretty(&json)
            .map_err(|e| DdlError::Other(e.to_string()))?);
        return Ok(());
    }

    println!("dulce-de-leche — ecosystem status");
    println!();

    for h in &health {
        println!("{}", diagnostics::format_health_line(h));
    }

    println!();
    let refs: Vec<&diagnostics::ToolHealth> = health.iter().collect();
    println!("{}", diagnostics::status_summary(refs));

    Ok(())
}

fn cmd_doctor(fix: bool, args: &dulce_de_leche::cli::Args) -> Result<()> {
    use dulce_de_leche::diagnostics;

    let ddl_dir = DdlDir::find_or_create().ok();

    if args.json {
        let messages = diagnostics::run_full_diagnostic(ddl_dir.as_ref(), fix)?;
        let json = serde_json::json!({
            "ok": true,
            "version": dulce_de_leche::VERSION,
            "data": { "diagnostics": messages },
            "warnings": [],
            "hints": []
        });
        println!("{}", serde_json::to_string_pretty(&json)
            .map_err(|e| DdlError::Other(e.to_string()))?);
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

fn cmd_version(check: bool, _args: &dulce_de_leche::cli::Args) -> Result<()> {
    println!("ddl: {}", dulce_de_leche::VERSION);
    println!();

    match DdlDir::find_or_create() {
        Ok(ddl_dir) => {
            let mut names: Vec<&String> = ddl_dir.manifest.tools.keys().collect();
            names.sort();
            for name in names {
                if let Some(entry) = ddl_dir.manifest.tools.get(name) {
                    println!("  {:12} v{} (via {})", name, entry.installed, entry.source);
                }
            }
        }
        Err(_) => {
            use dulce_de_leche::diagnostics;
            for tool in dulce_de_leche::platform::MANAGED_TOOLS {
                if diagnostics::which(tool.name).is_some() {
                    let version = diagnostics::get_tool_version(tool)
                        .unwrap_or_else(|| "?".to_string());
                    println!("  {:12} v{}", tool.name, version);
                }
            }
        }
    }

    if check {
        println!();
        println!("Checking for updates... (requires network)");
        println!("  ℹ  Online version check not yet implemented");
    }

    Ok(())
}

fn cmd_upgrade(tool_name: Option<&str>, _args: &dulce_de_leche::cli::Args) -> Result<()> {
    let platform = dulce_de_leche::platform::Platform::detect()
        .ok_or_else(|| DdlError::UnsupportedPlatform("Could not detect platform".to_string()))?;

    let mut ddl_dir = DdlDir::find_or_create()?;

    if let Some(name) = tool_name {
        let tool = dulce_de_leche::platform::find_tool(name)
            .ok_or_else(|| DdlError::ToolNotFound(name.to_string()))?;
        println!("Upgrading {}...", tool.name);
        let result = dulce_de_leche::installer::install_tool(tool, &platform, &mut ddl_dir.manifest, false);
        if result.success {
            ddl_dir.record_installed(result.tool, "unknown", &result.method.to_string())?;
        } else {
            ddl_dir.record_failed(result.tool, &result.method.to_string())?;
        }
        println!("  {} {}", if result.success { "✓" } else { "✗" }, result.message);
    } else {
        println!("Upgrading all tools...");
        let results = dulce_de_leche::installer::install_all_tools(&platform, &mut ddl_dir.manifest, false);
        for result in &results {
            if result.success {
                ddl_dir.record_installed(result.tool, "unknown", &result.method.to_string())?;
            } else {
                ddl_dir.record_failed(result.tool, &result.method.to_string())?;
            }
            let icon = if result.success { "✓" } else { "✗" };
            println!("  {} {}", icon, result.message);
        }
    }

    Ok(())
}

fn cmd_migrate(undo: bool, _args: &dulce_de_leche::cli::Args) -> Result<()> {
    let ddl_dir = DdlDir::find_or_create()?;

    if undo {
        println!("Undoing migration — restoring legacy config locations...");
        let migrated = dulce_de_leche::dot_ddl::migrated_tools(&ddl_dir);
        if migrated.is_empty() {
            println!("  No migrated configs found.");
            return Ok(());
        }
        for (tool_name, legacy_path) in &migrated {
            println!("  Restoring {}...", tool_name);
            ddl_dir.unmigrate_tool(tool_name, legacy_path)?;
            println!("  ✓ {} restored to {}", tool_name, legacy_path.display());
        }
        println!("Done.");
        return Ok(());
    }

    println!("Phase 1 migration — moving configs under .ddl/...");
    let legacy_configs = ddl_dir.detect_legacy_configs();
    if legacy_configs.is_empty() {
        println!("  No legacy configs found. Nothing to migrate.");
        return Ok(());
    }

    for (tool_name, legacy_path) in &legacy_configs {
        println!("  Migrating {} from {}...", tool_name, legacy_path.display());
        ddl_dir.migrate_tool(tool_name, legacy_path)?;
        println!("  ✓ {} migrated to {}", tool_name, ddl_dir.tool_path(tool_name).display());
    }

    let mut ddl_dir = ddl_dir;
    ddl_dir.manifest.migration_state = "phase1".to_string();
    ddl_dir.save_manifest()?;

    println!();
    println!("Done. All configs are now under .ddl/.");
    println!("Legacy locations are symlinks to .ddl/<tool>/ — everything is backwards compatible.");
    println!("Run `ddl migrate --undo` to restore the previous layout.");

    Ok(())
}

fn cmd_scope(_args: &dulce_de_leche::cli::Args) -> Result<()> {
    if let Some(ddl_dir) = dulce_de_leche::find_ddl_dir() {
        println!("{}", ddl_dir.display());
    } else {
        println!("No .ddl/ directory found (walked up from CWD)");
    }
    Ok(())
}