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
use crate::repo_status;
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

pub fn cmd_status(refresh: bool, args: &crate::cli::Args) -> Result<()> {
    use crate::diagnostics;
    use crate::find_ddl_dir;

    // --refresh (DDL-76lm): re-obtain the repo-status feeds and rewrite
    // the record via the write ordering. Requires the manifest; the
    // fast-path output below is unchanged without the flag.
    if refresh {
        return cmd_status_refresh(args);
    }

    let ddl_dir = DdlDir::find();
    let report = diagnostics::build_status_report(ddl_dir.as_ref());
    // Project-status section (DDL-852c task 2.1): read-only view of the
    // status record; never re-obtains feeds, never affects the exit code.
    let repo_root = std::env::current_dir().map_err(DdlError::Io)?;
    let project =
        diagnostics::build_project_status(&repo_root, ddl_dir.as_ref().map(|d| d.path.as_path()));

    if args.is_json() {
        let data = serde_json::json!({
            "sections": report.sections,
            "project": project,
        });
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

    // ── Project status (DDL-852c task 2.1) ──
    println!();
    println!("── Project status ──");
    // Contract precedence: the live record is the source of truth (the
    // in-band header is shared with the repo-status capability).
    println!("{}", project.precedence);
    if !project.record_present {
        println!("  no status record — run `ddl status --refresh`");
    } else {
        if let Some(generated) = &project.generated_at {
            println!("  record generated {generated}");
        }
        for feed in &project.feeds {
            match feed.staleness.as_deref() {
                Some(staleness) => {
                    let icon = if staleness == "fresh" { "✓" } else { "⚠" };
                    println!(
                        "  {icon} {:12}  observed {}  {}",
                        feed.name,
                        feed.observed_at.as_deref().unwrap_or("?"),
                        staleness
                    );
                }
                None => {
                    println!(
                        "  ○ {:12}  unavailable — {}",
                        feed.name,
                        feed.detail.as_deref().unwrap_or("no detail")
                    );
                }
            }
        }
        if project.may_be_stale {
            // Labeled as a heuristic (D1): the authoritative content-diff
            // verdict is computed only by `--refresh` and the doctor check.
            println!(
                "  ⚠ may be stale (heuristic: a tracked feed store was committed after the record was generated — not the authoritative verdict)"
            );
        }
        if let Some(suggestion) = &project.suggestion {
            println!("  → Run: {suggestion}");
        }
    }

    // health/discovery-mode-no-manifest + dot-ddl/manifest-missing: outside a
    // manifest-managed checkout (no `.ddl/` or no manifest.json), status only
    // discovers from PATH — point the user at `ddl init`.
    let manifest_present = find_ddl_dir()
        .map(|d| d.join("manifest.json").is_file())
        .unwrap_or(false);
    if !manifest_present {
        println!();
        println!("No manifest found in this directory — run `ddl init` to track tools here.");
    }

    Ok(())
}

/// The `ddl status --refresh` command path (DDL-76lm).
///
/// Without `.ddl/manifest.json` the refresh refuses (exit code 1 through
/// the standard error path — human "Error:" line or the JSON error
/// envelope) with `ddl init` guidance, and no record is created. With
/// it, the ordering runs under the manifest lock; human output reports
/// each step, JSON output emits the record as the envelope's `data`.
fn cmd_status_refresh(args: &crate::cli::Args) -> Result<()> {
    let ddl = match DdlDir::find() {
        Some(d) if d.path.join("manifest.json").is_file() => d,
        _ => {
            return Err(DdlError::Other(
                "no `.ddl/manifest.json` in this repository — run `ddl init` first".to_string(),
            ));
        }
    };

    let repo_root = std::env::current_dir().map_err(DdlError::Io)?;
    let report = repo_status::run_refresh(&repo_root, &ddl.path, &ddl.manifest)?;

    if args.is_json() {
        let data = serde_json::json!(report.record);
        let json_str =
            output::json_output(crate::VERSION, true, EnvelopeKind::Ok, data, vec![], vec![])?;
        println!("{json_str}");
        return Ok(());
    }

    for step in &report.steps {
        println!(
            "{} {}: {}",
            if step.ok { "✓" } else { "✗" },
            step.name,
            step.detail
        );
    }
    println!();
    println!(
        "status record written to {} — commit it so fresh clones and CI have orientation",
        repo_status::record_path(&ddl.path).display()
    );
    Ok(())
}

pub fn cmd_doctor(fix: bool, strict: bool, args: &crate::cli::Args) -> Result<()> {
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
        strict_violations(&report, strict);
        return Ok(());
    }

    println!("dulce-de-leche — diagnostics");
    println!();

    let messages = diagnostics::run_full_diagnostic(ddl_dir.as_ref(), fix)?;
    for msg in &messages {
        println!("  {}", msg);
    }

    if strict {
        // Reuse the structured report for the same enforcement rule (the
        // human path's text is not machine-parseable).
        let report = diagnostics::run_full_report(ddl_dir.as_ref(), fix)?;
        strict_violations(&report, strict);
    }

    Ok(())
}

/// `--strict` enforcement (DDL-9kz.2): exit non-zero when any REPO-SCOPE
/// diagnostic (`ddl.repo.*`, DDL-852c: including the status-pipeline
/// checks) is at warn/fail level. The report is always emitted first, so
/// gate consumers keep the structured evidence; the exit code is the
/// enforcement. Machine-scope findings (tool installs, nested tool
/// doctors) do not fail a strict run — the gate is repo hygiene: tracked
/// beads export, hooks wiring, gitignore, front door, session protocol,
/// status record freshness.
fn strict_violations(report: &crate::diagnostics::DiagnosticReport, strict: bool) {
    if !strict {
        return;
    }
    // Repo findings live under the `repo` key (DDL-852c, design D5) —
    // same enforcement rule, different home.
    let violations = report
        .repo
        .checks
        .iter()
        .filter(|d| d.level == "warn" || d.level == "fail")
        .count();
    if violations > 0 {
        eprintln!("doctor --strict: {violations} repo-hygiene finding(s)");
        std::process::exit(1);
    }
}

/// `ddl gates run` (DDL-9kz.6): run the repo gate detached. The run
/// mechanics live in [`dulce_de_leche::gates::run_gate`]; both formats
/// wait by default (bounded by `--wait-secs`) and exit with the gate's
/// status — JSON consumers get the handle/tail in the envelope instead of
/// interleaved raw output.
pub fn cmd_gates_run(
    detach: bool,
    wait_secs: u64,
    cmd: &[String],
    args: &crate::cli::Args,
) -> Result<()> {
    let repo_root = crate::installer::repo_root();
    let json = args.is_json();
    let opts = crate::gates::GateOpts::new(detach, wait_secs, !json);
    let outcome = crate::gates::run_gate(&repo_root, cmd, opts)?;
    render_gate_outcome(outcome, json, detach)
}

fn render_gate_outcome(outcome: crate::gates::GateOutcome, json: bool, detach: bool) -> Result<()> {
    match outcome {
        crate::gates::GateOutcome::Exited { code, handle } => {
            if json {
                print_gate_envelope(handle, Some(code))?;
            }
            if !json {
                if code == 0 {
                    eprintln!("gate passed");
                } else {
                    eprintln!("gate failed with exit {code} (tail above)");
                }
            }
            std::process::exit(code.clamp(0, 255));
        }
        crate::gates::GateOutcome::Released { handle } => {
            if json {
                return print_gate_envelope(handle, None);
            }
            if detach && handle.status == "running" {
                println!(
                    "{}",
                    serde_json::to_string(&handle).map_err(|e| DdlError::Other(e.to_string()))?
                );
                return Ok(());
            }
            eprintln!(
                "gate still running (pid {}) — released; child survives; log {}",
                handle.pid, handle.log_path
            );
            println!(
                "{}",
                serde_json::to_string(&handle).map_err(|e| DdlError::Other(e.to_string()))?
            );
            Ok(())
        }
    }
}

/// The `gates run` JSON envelope: the run handle plus the gate exit code
/// when the run finished inside the wait budget.
fn print_gate_envelope(handle: crate::gates::RunHandle, exit_code: Option<i32>) -> Result<()> {
    let mut data = serde_json::to_value(&handle).map_err(|e| DdlError::Other(e.to_string()))?;
    if let Some(code) = exit_code {
        data["exit_code"] = serde_json::json!(code);
    }
    let json_str = crate::output::json_output(
        crate::VERSION,
        true,
        genesis::envelope::EnvelopeKind::Info,
        data,
        vec![],
        vec![],
    )?;
    println!("{json_str}");
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
