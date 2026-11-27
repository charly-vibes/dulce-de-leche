//! `ddl init` gates wiring and resume-summary phases.

use super::phases::abort_json;
use super::*;

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
    match &report.sad {
        crate::gates::SadGateOutcome::Wired(_) => {
            println!("  ✓ sad gate: wired (sad hooks install)");
        }
        crate::gates::SadGateOutcome::NotInstalled => println!(
            "  ○ sad gate: sad not on PATH — skipped (install sad, then re-run `ddl init --gates`)"
        ),
        crate::gates::SadGateOutcome::Refused(note) => {
            println!("  ○ sad gate: `sad hooks install` refused — advisory: {note}");
        }
    }
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
    let session_label = match report.session_protocol {
        crate::installer::SessionProtocolChange::Written => "written",
        crate::installer::SessionProtocolChange::Rewritten => "rewritten (drift fixed)",
        crate::installer::SessionProtocolChange::Unchanged => "already present",
    };
    println!("  ✓ AGENTS.md: session-close protocol block {session_label}");
}

/// JSON-mode one-line gates summary.
fn gates_json_summary(report: &crate::gates::WiringReport) -> String {
    use crate::gates::{BeadsChange, Change, SadGateOutcome};
    format!(
        "gates wired: lefthook {}, sad {}, gitignore +{} entries, beads {}, export {}, staged +{}, session-protocol {}",
        match report.lefthook {
            Change::Created => "created",
            Change::Updated => "updated",
            Change::Unchanged => "unchanged",
        },
        match &report.sad {
            SadGateOutcome::Wired(_) => "wired",
            SadGateOutcome::NotInstalled => "skipped (sad not installed)",
            SadGateOutcome::Refused(_) => "refused (advisory)",
        },
        report.gitignore_added.len(),
        match report.beads {
            BeadsChange::Created => "created",
            BeadsChange::FlippedToNoDb => "flipped to no-db",
            BeadsChange::Unchanged => "unchanged",
        },
        if report.beads_exported { "yes" } else { "no" },
        report.beads_staged.len(),
        match report.session_protocol {
            crate::installer::SessionProtocolChange::Written => "written",
            crate::installer::SessionProtocolChange::Rewritten => "rewritten (drift fixed)",
            crate::installer::SessionProtocolChange::Unchanged => "unchanged",
        },
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
