//! CLI command implementations.
//;
//! Split from the former monolithic main.rs (pretender file_lines
//! gate). Each cmd_* is moved verbatim (or decomposed into
//! behavior-preserving phases); main.rs is now a thin dispatch shell.

mod init;
mod install;
mod migrate;
mod skills;
mod upgrade;
mod version;

pub use init::*;
pub use install::cmd_install;
pub use migrate::cmd_migrate;
pub use skills::{ensure_incitaciones_skills, install_skills_scope};
pub use upgrade::cmd_upgrade;
pub use version::cmd_version;

use crate::dot_ddl::DdlDir;
use crate::error::{DdlError, Result};
use crate::output;
use genesis::envelope::EnvelopeKind;

/// Map a feedback outcome to the (message, url) user-facing pair.
fn feedback_message(result: &genesis::feedback::gh::GhResult) -> (String, Option<String>) {
    match result {
        genesis::feedback::gh::GhResult::Created { url, number } => {
            (format!("Created issue #{number}: {url}"), Some(url.clone()))
        }
        genesis::feedback::gh::GhResult::FallbackUrl(url) => (
            format!("Could not create the issue directly — open {url} to file it manually"),
            Some(url.clone()),
        ),
        genesis::feedback::gh::GhResult::LocalFile(path) => (
            format!(
                "Network unavailable — issue body saved to {}",
                path.display()
            ),
            None,
        ),
    }
}

pub fn cmd_feedback(
    args: genesis::feedback::FeedbackArgs,
    args_cli: &crate::cli::Args,
) -> Result<()> {
    match genesis::feedback::handle_feedback(
        &args,
        "ddl",
        crate::VERSION,
        "charly-vibes/dulce-de-leche",
        &std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
    ) {
        Ok(result) => {
            let (message, url) = feedback_message(&result);
            if args_cli.is_json() {
                let data = serde_json::json!({
                    "message": message,
                    "url": url,
                });
                let json_str = output::json_output(
                    crate::VERSION,
                    true,
                    EnvelopeKind::Ok,
                    data,
                    vec![],
                    vec![],
                )?;
                println!("{json_str}");
            } else {
                output::print_success(&message, false);
            }
            Ok(())
        }
        Err(e) => Err(DdlError::Other(e)),
    }
}

pub fn cmd_catalog(args: &crate::cli::Args) -> Result<()> {
    use crate::catalog;

    let entries = catalog::entries();
    if args.is_json() {
        let data = serde_json::json!({ "count": entries.len(), "tools": entries });
        let json_str = output::json_output(
            crate::VERSION,
            true,
            EnvelopeKind::List,
            data,
            vec![],
            vec![],
        )?;
        println!("{json_str}");
        return Ok(());
    }

    print!("{}", catalog::render_human());
    Ok(())
}

pub fn cmd_status(args: &crate::cli::Args) -> Result<()> {
    use crate::diagnostics;

    let ddl_dir = DdlDir::find();
    let report = diagnostics::build_status_report(ddl_dir.as_ref());

    if args.is_json() {
        let data = serde_json::json!(report);
        let json_str = output::json_output(
            crate::VERSION,
            true,
            EnvelopeKind::List,
            data,
            vec![],
            vec![],
        )?;
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

pub fn cmd_doctor(fix: bool, args: &crate::cli::Args) -> Result<()> {
    use crate::diagnostics;

    let ddl_dir = DdlDir::find();

    if args.is_json() {
        let report = diagnostics::run_full_report(ddl_dir.as_ref(), fix)?;
        let data = serde_json::json!(report);
        let json_str = output::json_output(
            crate::VERSION,
            true,
            EnvelopeKind::Doctor,
            data,
            vec![],
            vec![],
        )?;
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

pub fn cmd_scope(args: &crate::cli::Args) -> Result<()> {
    if let Some(ddl_dir) = crate::find_ddl_dir() {
        if args.is_json() {
            let data = serde_json::json!({"ddl_dir": ddl_dir.display().to_string()});
            let json_str = output::json_output(
                crate::VERSION,
                true,
                EnvelopeKind::Info,
                data,
                vec![],
                vec![],
            )?;
            println!("{json_str}");
        } else {
            println!("{}", ddl_dir.display());
        }
    } else if args.is_json() {
        let data = serde_json::json!({"ddl_dir": null});
        let json_str = output::json_output(
            crate::VERSION,
            true,
            EnvelopeKind::Info,
            data,
            vec![],
            vec![],
        )?;
        println!("{json_str}");
    } else {
        println!("No .ddl/ directory found (walked up from CWD)");
    }
    Ok(())
}

/// Detect the host platform, mapping failure to the standard unsupported
/// error all commands share.
pub fn detect_platform() -> Result<crate::platform::Platform> {
    crate::platform::Platform::detect()
        .ok_or_else(|| DdlError::UnsupportedPlatform("Could not detect platform".to_string()))
}

/// JSON-mode envelope collector, or `None` in human mode. Collects all
/// output data and emits a single envelope on drop (handles all return
/// paths including early returns).
pub fn json_guard_or_none(args: &crate::cli::Args) -> Option<output::JsonCollectorGuard> {
    if args.is_json() {
        Some(output::JsonCollectorGuard::start(
            crate::VERSION,
            genesis::envelope::EnvelopeKind::List,
        ))
    } else {
        None
    }
}

/// Print the detected-platform banner (human mode only).
fn print_platform_banner(platform: &crate::platform::Platform, args: &crate::cli::Args) {
    if args.is_json() {
        return;
    }
    println!("Detected platform: {} ({})", platform.os, platform.arch);
    let cargo_fallback = if crate::platform::PackageManager::Cargo.is_available() {
        "available"
    } else {
        "not installed"
    };
    println!("Install method: binary download (primary); cargo fallback: {cargo_fallback}");
}
