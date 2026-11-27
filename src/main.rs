//! dulce-de-leche (ddl) — CLI entry point.
//!
//! Thin dispatch shell: the command implementations live in the
//! [`dulce_de_leche::commands`] module (split out for the pretender
//! file_lines gate).

use std::process;

use dulce_de_leche::output;
use genesis::cli::maybe_print_version_json;

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
        // Record the failure for `ddl feedback bug --from-last-error`.
        genesis::feedback::scratch::write_scratch_best_effort(
            "ddl",
            &genesis::feedback::scratch::ErrorRecord {
                ts: chrono::Utc::now().to_rfc3339(),
                argv: std::env::args().collect(),
                exit: 1,
                footer: None,
                kind: "error".to_string(),
            },
        );
        output::print_error(&format!("{e}"), want_json);
        process::exit(1);
    }
}

fn run(args: dulce_de_leche::cli::Args) -> dulce_de_leche::error::Result<()> {
    use clap::CommandFactory;
    use dulce_de_leche::cli::Commands;
    use dulce_de_leche::commands;

    match args.command {
        Some(Commands::Init {
            ref tools,
            no_install,
            gates,
        }) => commands::cmd_init(tools.clone(), no_install, gates, &args),
        Some(Commands::Install { ref tool }) => commands::cmd_install(tool, &args),
        Some(Commands::Catalog) => commands::cmd_catalog(&args),
        Some(Commands::Feedback {
            ref kind,
            from_last_error,
            dry_run,
            ref title,
        }) => {
            let mut fb = genesis::feedback::FeedbackArgs::new(kind, dry_run, from_last_error);
            if let Some(t) = title {
                fb = fb.with_title(t);
            }
            commands::cmd_feedback(fb, &args)
        }
        Some(Commands::Status) => commands::cmd_status(&args),
        Some(Commands::Gates { ref cmd }) => match cmd {
            dulce_de_leche::cli::GatesCmd::Run {
                detach,
                wait_secs,
                cmd,
            } => commands::cmd_gates_run(*detach, *wait_secs, cmd, &args),
        },
        Some(Commands::Doctor { fix, strict }) => commands::cmd_doctor(fix, strict, &args),
        Some(Commands::Version { check }) => commands::cmd_version(check, &args),
        Some(Commands::Upgrade { ref tool }) => commands::cmd_upgrade(tool.as_deref(), &args),
        Some(Commands::Migrate { undo }) => commands::cmd_migrate(undo, &args),
        Some(Commands::Scope) => commands::cmd_scope(&args),
        Some(Commands::Setup {
            ref tools,
            no_install,
        }) => dulce_de_leche::cli::Args::cmd_setup(tools.clone(), no_install, args.yes, &args),
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
