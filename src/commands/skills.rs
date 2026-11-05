//! incitaciones skills install helpers for `ddl init`.

use crate::error::{DdlError, Result};
use std::io::IsTerminal;

/// Global skills (~/.agents/skills/) are the ddl-preferred scope. When
/// missing and `prompt_eligible` (incitaciones is in scope for this init):
/// interactive sessions prompt for global, local, or skip; `--yes` installs
/// globally; JSON mode is check-only. When not eligible, only a hint is
/// printed so users who deselected the tool aren't re-prompted.
pub fn ensure_incitaciones_skills(args: &crate::cli::Args, prompt_eligible: bool) -> Result<()> {
    let status = crate::skills::skill_status();
    if status.has_global() {
        if !args.is_json() {
            let ver = status.global_version.unwrap_or_else(|| "?".to_string());
            println!("  ✓ incitaciones skills installed globally (npm:{ver})");
        }
        return Ok(());
    }

    if args.is_json() {
        // Check-only in JSON mode — no prompts, no state changes.
        return Ok(());
    }

    if args.yes {
        // Non-interactive: default to the global scope.
        install_skills_scope(true, args)?;
        return Ok(());
    }

    if !prompt_eligible {
        println!(
            "  ℹ  incitaciones skills not installed globally — install with: npx incitaciones install --global"
        );
        return Ok(());
    }

    // Only prompt when the session is actually interactive: a non-TTY
    // session (CI, piped output) that reaches this point because the tool is
    // eligible-but-uninstalled must get the deterministic hint below instead
    // of a cliclack "not connected" failure (DDL-fw7's manifest reconcile can
    // make previously untracked tools eligible without any interactive ask).
    if !std::io::stdin().is_terminal() {
        println!(
            "  ℹ  incitaciones skills not installed globally — install with: npx incitaciones install --global"
        );
        return Ok(());
    }

    let local_note = if status.local_version.is_some() {
        " A local install exists in this project."
    } else {
        ""
    };
    let choice: String = cliclack::select(format!(
        "incitaciones skills are not installed globally.{local_note} Install them?"
    ))
    .item(
        "global",
        "Global",
        "~/.agents/skills/ — available in all projects (recommended)",
    )
    .item("local", "Local", ".agents/skills/ — this project only")
    .item("skip", "Skip", "Don't install skills now")
    .interact()
    .map_err(|e| DdlError::Other(format!("Selection cancelled: {e}")))?
    .to_string();

    match choice.as_str() {
        "global" => install_skills_scope(true, args),
        "local" => install_skills_scope(false, args),
        _ => {
            println!("  ℹ  Skipped. Install later with: npx incitaciones install --global");
            Ok(())
        }
    }
}

/// Install incitaciones skills in the given scope, reporting the outcome.
/// Failures are advisory — they don't fail the whole init.
pub fn install_skills_scope(global: bool, args: &crate::cli::Args) -> Result<()> {
    let scope = if global { "global" } else { "local" };
    if !args.is_json() {
        println!("  Installing incitaciones skills ({scope})...");
    }
    match crate::skills::install_skills(global, args.verbose_quiet.raw_count() >= 1) {
        Ok(()) => {
            if !args.is_json() {
                println!("  ✓ incitaciones skills installed ({scope})");
            }
            Ok(())
        }
        Err(e) => {
            if !args.is_json() {
                println!(
                    "  ⚠ incitaciones skill install failed: {e} — install later with: npx incitaciones install --{scope}"
                );
            }
            Ok(())
        }
    }
}
