//! dulce-de-leche (ddl) — CLI entry point.

use clap::CommandFactory;
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
        Some(Commands::Status) => {
            cmd_status(&args)
        }
        Some(Commands::Doctor { fix }) => {
            cmd_doctor(fix, &args)
        }
        Some(Commands::Version { check }) => {
            cmd_version(check, &args)
        }
        Some(Commands::Upgrade { ref tool }) => {
            cmd_upgrade(tool.as_deref(), &args)
        }
        Some(Commands::Migrate { undo }) => {
            cmd_migrate(undo, &args)
        }
        Some(Commands::Scope) => {
            cmd_scope(&args)
        }
        None => {
            // No subcommand — show help
            dulce_de_leche::cli::Args::command()
                .print_help()
                .ok();
            println!();
            Ok(())
        }
    }
}

fn cmd_init(
    _tools: Option<String>,
    _no_install: bool,
    _args: &dulce_de_leche::cli::Args,
) -> dulce_de_leche::error::Result<()> {
    eprintln!("ddl init — not yet implemented");
    eprintln!("See https://github.com/charly-vibes/dulce-de-leche for progress");
    Ok(())
}

fn cmd_install(
    _tool: &str,
    _args: &dulce_de_leche::cli::Args,
) -> dulce_de_leche::error::Result<()> {
    eprintln!("ddl install — not yet implemented");
    Ok(())
}

fn cmd_status(
    _args: &dulce_de_leche::cli::Args,
) -> dulce_de_leche::error::Result<()> {
    eprintln!("ddl status — not yet implemented");
    Ok(())
}

fn cmd_doctor(
    _fix: bool,
    _args: &dulce_de_leche::cli::Args,
) -> dulce_de_leche::error::Result<()> {
    eprintln!("ddl doctor — not yet implemented");
    Ok(())
}

fn cmd_version(
    _check: bool,
    _args: &dulce_de_leche::cli::Args,
) -> dulce_de_leche::error::Result<()> {
    println!("ddl {}", dulce_de_leche::VERSION);
    Ok(())
}

fn cmd_upgrade(
    _tool: Option<&str>,
    _args: &dulce_de_leche::cli::Args,
) -> dulce_de_leche::error::Result<()> {
    eprintln!("ddl upgrade — not yet implemented");
    Ok(())
}

fn cmd_migrate(
    _undo: bool,
    _args: &dulce_de_leche::cli::Args,
) -> dulce_de_leche::error::Result<()> {
    eprintln!("ddl migrate — not yet implemented");
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