//! dulce-de-leche (ddl) — CLI entry point.

use clap::CommandFactory;
use std::io::Write;
use std::path::PathBuf;
use std::process;

fn main() {
    let args = dulce_de_leche::cli::Args::parse_or_exit();

    if let Err(e) = run(args) {
        eprintln!("{e:?}");
        process::exit(1);
    }
}

fn run(args: dulce_de_leche::cli::Args) -> dulce_de_leche::error::Result<()> {
    use dulce_de_leche::cli::Commands;

    match args.command {
        Some(Commands::Init { ref tools, no_install }) => {
            cmd_init(tools.clone(), no_install, &args)
        }
        Some(Commands::Install { ref tool }) => {
            cmd_install(tool, &args)
        }
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

fn cmd_init(
    tools: Option<String>,
    no_install: bool,
    args: &dulce_de_leche::cli::Args,
) -> dulce_de_leche::error::Result<()> {
    use dulce_de_leche::installer::InstallMethod;

    let platform = dulce_de_leche::platform::Platform::detect()
        .ok_or_else(|| dulce_de_leche::error::DdlError::UnsupportedPlatform(
            "Could not detect platform".to_string(),
        ))?;

    println!("╭──────────────────────────────────────╮");
    println!("│  dulce-de-leche — charly-vibes       │");
    println!("│  bundle orchestrator v{}       │", dulce_de_leche::VERSION);
    println!("╰──────────────────────────────────────╯");
    println!();
    println!("Detected platform: {} ({})", platform.os, platform.arch);

    // Check package manager availability
    let pm = match platform.os {
        dulce_de_leche::platform::Os::Macos => {
            if InstallMethod::Brew.check_prerequisites().is_ok() {
                println!("Available package manager: brew");
                "brew"
            } else {
                println!("Available package manager: cargo (binary download fallback)");
                "cargo/binary"
            }
        }
        dulce_de_leche::platform::Os::Linux => {
            println!("Available package manager: cargo (binary download fallback)");
            "cargo/binary"
        }
        dulce_de_leche::platform::Os::Windows => {
            if InstallMethod::Scoop.check_prerequisites().is_ok() {
                println!("Available package manager: scoop");
                "scoop"
            } else {
                println!("Available package manager: binary download");
                "binary"
            }
        }
    };
    let _ = pm;

    // Determine which tools to install
    let selected_tools: Vec<String> = if let Some(ref tools_str) = tools {
        tools_str.split(',').map(|s| s.trim().to_string()).collect()
    } else {
        dulce_de_leche::platform::MANAGED_TOOLS
            .iter()
            .map(|t| t.name.to_string())
            .collect()
    };

    if no_install {
        println!("Skipping installation (--no-install)");
    } else {
        println!("Installing {} tools...", selected_tools.len());
        println!();

        // Load or create manifest
        let ddl_dir = dulce_de_leche::find_ddl_dir()
            .unwrap_or_else(|| PathBuf::from(".ddl"));
        let manifest_path = ddl_dir.join("manifest.json");
        let mut manifest = dulce_de_leche::manifest::Manifest::load(&manifest_path)?;

        let results = if tools.is_some() {
            dulce_de_leche::installer::install_selected_tools(
                &selected_tools,
                &platform,
                &mut manifest,
                args.verbose,
            )
        } else {
            dulce_de_leche::installer::install_all_tools(
                &platform,
                &mut manifest,
                args.verbose,
            )
        };

        println!();
        for result in &results {
            let icon = if result.success { "✓" } else { "✗" };
            println!("  {} {}", icon, result.message);
        }

        // Save manifest
        std::fs::create_dir_all(&ddl_dir)
            .map_err(|e| dulce_de_leche::error::DdlError::Io(e))?;
        manifest.save(&manifest_path)?;

        println!();
        // Run init commands for successfully installed tools
        for result in &results {
            if result.success {
                let tool = dulce_de_leche::platform::find_tool(result.tool);
                if let Some(t) = tool {
                    let _ = dulce_de_leche::installer::run_tool_init(t, args.verbose);
                }
            }
        }
    }

    // Check if .ddl/ exists, create if not
    let ddl_dir = PathBuf::from(".ddl");
    if !ddl_dir.exists() {
        std::fs::create_dir_all(&ddl_dir)
            .map_err(|e| dulce_de_leche::error::DdlError::Io(e))?;
        println!("  ✓ .ddl/ created");
    }

    // Gitignore integration
    let gitignore_path = PathBuf::from(".gitignore");
    if gitignore_path.exists() {
        let contents = std::fs::read_to_string(&gitignore_path)
            .map_err(|e| dulce_de_leche::error::DdlError::Io(e))?;
        if !contents.contains(".ddl/**/*.db") {
            if args.yes {
                // Auto-add gitignore entries
                let additions = "\n\n# dulce-de-leche — data files\n.ddl/**/*.db\n.ddl/**/store/\n.ddl/install-log.json\n.ddl/doctor-cache.json\n.ddl/compatibility-cache.json\n";
                std::fs::OpenOptions::new()
                    .append(true)
                    .open(&gitignore_path)
                    .map_err(|e| dulce_de_leche::error::DdlError::Io(e))?
                    .write_all(additions.as_bytes())
                    .map_err(|e| dulce_de_leche::error::DdlError::Io(e))?;
                println!("  ✓ .gitignore updated");
            } else {
                println!("  ℹ  Run `ddl init --yes` to auto-add .gitignore entries");
            }
        }
    }

    println!();
    println!("Done! Run `ddl status` to verify everything.");
    Ok(())
}

fn cmd_install(
    tool_name: &str,
    args: &dulce_de_leche::cli::Args,
) -> dulce_de_leche::error::Result<()> {
    let platform = dulce_de_leche::platform::Platform::detect()
        .ok_or_else(|| dulce_de_leche::error::DdlError::UnsupportedPlatform(
            "Could not detect platform".to_string(),
        ))?;

    let tool = dulce_de_leche::platform::find_tool(tool_name)
        .ok_or_else(|| {
            // Try "did you mean"
            let names: Vec<&str> = dulce_de_leche::platform::MANAGED_TOOLS
                .iter().map(|t| t.name).collect();
            let suggestion = dulce_de_leche::platform::did_you_mean(tool_name, &names);
            dulce_de_leche::error::DdlError::ToolNotFound(format!(
                "{}",
                if let Some(s) = suggestion {
                    format!("Unknown tool '{}'. Did you mean '{}'?", tool_name, s)
                } else {
                    format!("Unknown tool '{}'", tool_name)
                }
            ))
        })?;

    let ddl_dir = dulce_de_leche::find_ddl_dir()
        .unwrap_or_else(|| PathBuf::from(".ddl"));
    let manifest_path = ddl_dir.join("manifest.json");
    let mut manifest = dulce_de_leche::manifest::Manifest::load(&manifest_path)?;

    // Check if already installed
    if dulce_de_leche::installer::is_tool_installed(tool.name) {
        println!("{} is already installed", tool.name);
        if let Some(entry) = manifest.get_tool(tool.name) {
            println!("  Version: {} (source: {})", entry.installed, entry.source);
        }
        println!("  Run `ddl upgrade {}` to update", tool.name);
        return Ok(());
    }

    println!("Installing {}...", tool.name);
    let result = dulce_de_leche::installer::install_tool(tool, &platform, &mut manifest, args.verbose);

    // Save manifest
    std::fs::create_dir_all(&ddl_dir)
        .map_err(|e| dulce_de_leche::error::DdlError::Io(e))?;
    manifest.save(&manifest_path)?;

    if result.success {
        println!("  ✓ {}", result.message);
        let _ = dulce_de_leche::installer::run_tool_init(tool, args.verbose);
        Ok(())
    } else {
        Err(dulce_de_leche::error::DdlError::InstallFailed(result.message))
    }
}

fn cmd_status(
    _args: &dulce_de_leche::cli::Args,
) -> dulce_de_leche::error::Result<()> {
    use dulce_de_leche::installer::is_tool_installed;

    println!("dulce-de-leche — ecosystem status");
    println!();

    let ddl_dir = dulce_de_leche::find_ddl_dir();
    let manifest = ddl_dir
        .as_ref()
        .and_then(|d| dulce_de_leche::manifest::Manifest::load(&d.join("manifest.json")).ok());

    let mut any_issues = false;

    for tool in dulce_de_leche::platform::MANAGED_TOOLS {
        let installed = is_tool_installed(tool.name);
        let version = if installed {
            dulce_de_leche::installer::get_tool_version(tool.name)
        } else {
            None
        };
        let manifest_entry = manifest.as_ref().and_then(|m| m.get_tool(tool.name));

        let status = match (installed, manifest_entry) {
            (true, Some(entry)) if entry.status == "installed" => {
                if let Some(v) = &version {
                    format!("✓ v{} (via {})", v, entry.source)
                } else {
                    format!("✓ installed (via {})", entry.source)
                }
            }
            (true, None) => {
                any_issues = true;
                format!("⚠ installed (no manifest — run `ddl init --no-install`)")
            }
            (true, Some(entry)) if entry.status == "failed" => {
                any_issues = true;
                format!("⚠ previously failed — run `ddl install {}` to retry", tool.name)
            }
            (false, Some(_)) => {
                any_issues = true;
                format!("✗ recorded in manifest but not found on PATH — run `ddl install {}`", tool.name)
            }
            (false, None) => {
                format!("○ not installed")
            }
            _ => {
                any_issues = true;
                format!("⚠ unknown state")
            }
        };

        println!("  {:12} {}", tool.name, status);
    }

    println!();
    if any_issues {
        println!("Some tools need attention. Run `ddl doctor` for details.");
    } else {
        println!("All tools are configured. ✓");
    }

    Ok(())
}

fn cmd_doctor(
    _fix: bool,
    _args: &dulce_de_leche::cli::Args,
) -> dulce_de_leche::error::Result<()> {
    println!("dulce-de-leche — diagnostics");
    println!();

    // Check platform
    match dulce_de_leche::platform::Platform::detect() {
        Some(p) => println!("  ✓ Platform: {} ({})", p.os, p.arch),
        None => println!("  ✗ Could not detect platform"),
    }

    // Check prerequisites
    let checks = [
        ("curl", dulce_de_leche::installer::is_tool_installed("curl")),
        ("wget", dulce_de_leche::installer::is_tool_installed("wget")),
        ("cargo", dulce_de_leche::installer::is_tool_installed("cargo")),
        ("brew", dulce_de_leche::installer::is_tool_installed("brew")),
        ("scoop", dulce_de_leche::installer::is_tool_installed("scoop")),
    ];

    for (name, found) in &checks {
        if *found {
            println!("  ✓ {} found on PATH", name);
        }
    }

    // Check .ddl/ directory
    match dulce_de_leche::find_ddl_dir() {
        Some(d) => {
            println!("  ✓ .ddl/ at {}", d.display());
            let manifest_path = d.join("manifest.json");
            match dulce_de_leche::manifest::Manifest::load(&manifest_path) {
                Ok(m) => {
                    println!("  ✓ Manifest loaded ({} tools tracked)", m.tools.len());
                    for (name, entry) in &m.tools {
                        let status_icon = if entry.status == "installed" { "✓" } else { "✗" };
                        println!("    {} {} (v{}, {})", status_icon, name, entry.installed, entry.source);
                    }
                }
                Err(e) => println!("  ✗ Failed to load manifest: {e}"),
            }
        }
        None => {
            println!("  ○ No .ddl/ directory found");
            println!("    Run `ddl init` to create one");
        }
    }

    println!();
    println!("Run `ddl doctor --fix` to attempt auto-fix of detected issues.");
    Ok(())
}

fn cmd_version(
    check: bool,
    _args: &dulce_de_leche::cli::Args,
) -> dulce_de_leche::error::Result<()> {
    println!("ddl: {}", dulce_de_leche::VERSION);
    println!();

    let ddl_dir = dulce_de_leche::find_ddl_dir();
    let manifest = ddl_dir
        .as_ref()
        .and_then(|d| dulce_de_leche::manifest::Manifest::load(&d.join("manifest.json")).ok());

    if let Some(m) = manifest {
        let mut tool_names: Vec<&String> = m.tools.keys().collect();
        tool_names.sort();
        for name in tool_names {
            if let Some(entry) = m.tools.get(name) {
                println!("  {:12} v{} (via {})", name, entry.installed, entry.source);
            }
        }
    } else {
        // Check installed tools via PATH
        for tool in dulce_de_leche::platform::MANAGED_TOOLS {
            if dulce_de_leche::installer::is_tool_installed(tool.name) {
                let version = dulce_de_leche::installer::get_tool_version(tool.name)
                    .unwrap_or_else(|| "?".to_string());
                println!("  {:12} v{}", tool.name, version);
            }
        }
    }

    if check {
        println!();
        println!("Checking for updates... (requires network)");
        // TODO: implement online version check
        println!("  ℹ  Online version check not yet implemented");
    }

    Ok(())
}

fn cmd_upgrade(
    tool_name: Option<&str>,
    _args: &dulce_de_leche::cli::Args,
) -> dulce_de_leche::error::Result<()> {
    let platform = dulce_de_leche::platform::Platform::detect()
        .ok_or_else(|| dulce_de_leche::error::DdlError::UnsupportedPlatform(
            "Could not detect platform".to_string(),
        ))?;

    let ddl_dir = dulce_de_leche::find_ddl_dir()
        .unwrap_or_else(|| PathBuf::from(".ddl"));
    let manifest_path = ddl_dir.join("manifest.json");
    let mut manifest = dulce_de_leche::manifest::Manifest::load(&manifest_path)?;

    if let Some(name) = tool_name {
        // Upgrade a single tool
        let tool = dulce_de_leche::platform::find_tool(name)
            .ok_or_else(|| dulce_de_leche::error::DdlError::ToolNotFound(name.to_string()))?;

        println!("Upgrading {}...", tool.name);
        let result = dulce_de_leche::installer::install_tool(tool, &platform, &mut manifest, false);
        println!("  {} {}", if result.success { "✓" } else { "✗" }, result.message);
    } else {
        // Upgrade all tools
        println!("Upgrading all tools...");
        let results = dulce_de_leche::installer::install_all_tools(&platform, &mut manifest, false);
        for result in &results {
            let icon = if result.success { "✓" } else { "✗" };
            println!("  {} {}", icon, result.message);
        }
    }

    std::fs::create_dir_all(&ddl_dir)
        .map_err(|e| dulce_de_leche::error::DdlError::Io(e))?;
    manifest.save(&manifest_path)?;
    Ok(())
}

fn cmd_migrate(
    _undo: bool,
    _args: &dulce_de_leche::cli::Args,
) -> dulce_de_leche::error::Result<()> {
    eprintln!("ddl migrate — not yet implemented");
    eprintln!("Phase 1 migration (symlink farm) will be available in a future release");
    Ok(())
}

fn cmd_scope(
    _args: &dulce_de_leche::cli::Args,
) -> dulce_de_leche::error::Result<()> {
    if let Some(ddl_dir) = dulce_de_leche::find_ddl_dir() {
        println!("{}", ddl_dir.display());
    } else {
        println!("No .ddl/ directory found (walked up from CWD)");
    }
    Ok(())
}