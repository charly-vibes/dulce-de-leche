//! `ddl upgrade` — single-tool and fleet upgrades.

use super::*;

/// Upgrade one explicitly named tool.
fn upgrade_one(
    name: &str,
    platform: &crate::platform::Platform,
    ddl_dir: &mut DdlDir,
    args: &crate::cli::Args,
) -> Result<()> {
    // DDL-6zn.8: shared resolver — aliases (espectacular→ah) and self-names
    // (dulce→teachable repo error) behave like `install`.
    let tool = crate::platform::resolve_tool(name).map_err(DdlError::ToolNotFound)?;
    if !args.is_json() {
        println!("Upgrading {}...", tool.name);
    }
    let result = crate::installer::upgrade_tool(
        tool,
        platform,
        &mut ddl_dir.manifest,
        args.verbose_quiet.raw_count() >= 1,
    );
    ddl_dir.save_manifest()?;
    output::print_install_result(result.success, result.tool, &result.message, args.is_json());
    if !result.success {
        return Err(DdlError::InstallFailed(result.message));
    }
    Ok(())
}

/// Tally one fleet-upgrade result into (success, failed, skipped) counts.
fn tally_result(
    result: &crate::installer::InstallResult,
    counts: &mut (u32, u32, u32),
    is_json: bool,
) {
    match result.method {
        crate::installer::InstallMethod::Skipped => counts.2 += 1,
        _ if result.success => counts.0 += 1,
        _ => counts.1 += 1,
    }
    output::print_install_result(result.success, result.tool, &result.message, is_json);
}

/// Build the fleet-upgrade summary line (non-JSON mode) — pure for tests.
fn upgrade_summary_line(counts: (u32, u32, u32)) -> String {
    let (success_count, fail_count, skipped_count) = counts;
    if fail_count > 0 && success_count == 0 && skipped_count == 0 {
        "All upgrades failed. Check network connectivity.".to_string()
    } else if fail_count > 0 {
        format!("{success_count} upgraded, {fail_count} failed, {skipped_count} up to date")
    } else if skipped_count > 0 {
        format!("All {skipped_count} tools are already up to date.")
    } else {
        format!("All {success_count} tools upgraded successfully.")
    }
}

/// Print the fleet-upgrade summary line (non-JSON mode).
fn print_upgrade_summary(counts: (u32, u32, u32)) {
    println!();
    println!("{}", upgrade_summary_line(counts));
}

/// Exit code for a finished fleet upgrade — pure for tests.
/// All failures (no successes, no skips) is a hard error; any mixed failure
/// is a partial failure (exit code 1).
fn fleet_upgrade_exit(counts: (u32, u32, u32)) -> Result<()> {
    let (success_count, fail_count, skipped_count) = counts;
    if fail_count > 0 && success_count == 0 && skipped_count == 0 {
        return Err(DdlError::InstallFailed("All upgrades failed".to_string()));
    }
    if fail_count > 0 {
        return Err(DdlError::PartialFailure);
    }
    Ok(())
}

/// Upgrade every managed tool, recording results and summarizing.
fn upgrade_all(
    platform: &crate::platform::Platform,
    ddl_dir: &mut DdlDir,
    args: &crate::cli::Args,
) -> Result<()> {
    if !args.is_json() {
        println!("Upgrading all tools...");
    }
    let results = crate::installer::upgrade_all_tools(
        platform,
        &mut ddl_dir.manifest,
        args.verbose_quiet.raw_count() >= 1,
    );
    ddl_dir.save_manifest()?;

    let mut counts = (0u32, 0u32, 0u32);
    for result in &results {
        tally_result(result, &mut counts, args.is_json());
    }

    if !args.is_json() {
        print_upgrade_summary(counts);
    }

    fleet_upgrade_exit(counts)
}

pub fn cmd_upgrade(tool_name: Option<&str>, args: &crate::cli::Args) -> Result<()> {
    let platform = detect_platform()?;
    let mut ddl_dir = DdlDir::find_or_create()?;

    if let Some(name) = tool_name {
        upgrade_one(name, &platform, &mut ddl_dir, args)
    } else {
        upgrade_all(&platform, &mut ddl_dir, args)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::DdlError;
    use crate::installer::{InstallMethod, InstallResult};

    fn result(success: bool, method: InstallMethod) -> InstallResult {
        InstallResult {
            tool: "wai",
            success,
            method,
            version: "2026.11.17".to_string(),
            message: "ok".to_string(),
        }
    }

    #[test]
    fn test_tally_counts_success_failed_and_skipped() {
        let mut counts = (0u32, 0u32, 0u32);
        tally_result(&result(true, InstallMethod::Cargo), &mut counts, true);
        tally_result(&result(false, InstallMethod::Binary), &mut counts, true);
        tally_result(&result(false, InstallMethod::Skipped), &mut counts, true);
        assert_eq!(
            counts,
            (1, 1, 1),
            "success/failure/skip must tally separately"
        );
    }

    #[test]
    fn test_upgrade_summary_reports_partial_failure_counts() {
        assert_eq!(
            upgrade_summary_line((2, 1, 0)),
            "2 upgraded, 1 failed, 0 up to date"
        );
    }

    #[test]
    fn test_upgrade_summary_reports_all_up_to_date() {
        assert_eq!(
            upgrade_summary_line((0, 0, 5)),
            "All 5 tools are already up to date."
        );
    }

    #[test]
    fn test_upgrade_summary_reports_all_failed_with_network_hint() {
        assert_eq!(
            upgrade_summary_line((0, 3, 0)),
            "All upgrades failed. Check network connectivity."
        );
    }

    #[test]
    fn test_fleet_exit_partial_failure_is_error_code_one() {
        let err = fleet_upgrade_exit((1, 1, 0)).unwrap_err();
        assert!(
            matches!(err, DdlError::PartialFailure),
            "mixed failure must map to PartialFailure (exit 1), got {err:?}"
        );
    }

    #[test]
    fn test_fleet_exit_all_failed_is_install_failed() {
        let err = fleet_upgrade_exit((0, 2, 0)).unwrap_err();
        assert!(matches!(err, DdlError::InstallFailed(_)));
    }

    #[test]
    fn test_fleet_exit_all_skipped_is_success() {
        fleet_upgrade_exit((0, 0, 3)).expect("no upgrades needed must exit Ok");
    }
}
