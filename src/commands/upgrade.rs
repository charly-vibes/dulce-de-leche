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

/// Print the fleet-upgrade summary line (non-JSON mode).
fn print_upgrade_summary(counts: (u32, u32, u32)) {
    let (success_count, fail_count, skipped_count) = counts;
    println!();
    if fail_count > 0 && success_count == 0 && skipped_count == 0 {
        println!("All upgrades failed. Check network connectivity.");
    } else if fail_count > 0 {
        println!("{success_count} upgraded, {fail_count} failed, {skipped_count} up to date");
    } else if skipped_count > 0 {
        println!("All {skipped_count} tools are already up to date.");
    } else {
        println!("All {success_count} tools upgraded successfully.");
    }
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

    let (success_count, fail_count, skipped_count) = counts;
    if fail_count > 0 && success_count == 0 && skipped_count == 0 {
        return Err(DdlError::InstallFailed("All upgrades failed".to_string()));
    }
    if fail_count > 0 {
        return Err(DdlError::PartialFailure);
    }
    Ok(())
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
