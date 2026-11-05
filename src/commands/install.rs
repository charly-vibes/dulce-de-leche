//! `ddl install <tool>` — single-tool install.

use super::*;

/// Handle the already-on-PATH case: report the tracked version, or probe and
/// record an untracked install (DDL-zw4) so manifest-driven paths
/// (status, version --json) see it. Source "skipped" matches the init
/// already-installed flow. Returns true when the install can be skipped.
fn already_installed(
    tool: &crate::platform::Tool,
    ddl_dir: &mut DdlDir,
    args: &crate::cli::Args,
) -> Result<bool> {
    if !crate::installer::is_tool_installed(tool.name) {
        return Ok(false);
    }
    if let Some(entry) = ddl_dir.manifest.get_tool(tool.name) {
        let msg = format!(
            "{} v{} is already installed (via {})",
            tool.name, entry.installed, entry.source
        );
        output::print_success(&msg, args.is_json());
    } else {
        let ver = crate::installer::get_installed_version(tool.name)
            .unwrap_or_else(|| "unknown".to_string());
        ddl_dir.record_installed(tool.name, &ver, "skipped")?;
        let msg = format!("{} v{} is already installed on PATH", tool.name, ver);
        output::print_success(&msg, args.is_json());
    }
    Ok(true)
}

/// Run a fresh install: invoke the installer, record the outcome, and run
/// the tool's init cascade on success.
fn fresh_install(
    tool: &crate::platform::Tool,
    platform: &crate::platform::Platform,
    ddl_dir: &mut DdlDir,
    args: &crate::cli::Args,
) -> Result<()> {
    if !args.is_json() {
        println!("Installing {}...", tool.name);
    }
    let result = crate::installer::install_tool(
        tool,
        platform,
        &mut ddl_dir.manifest,
        args.verbose_quiet.raw_count() >= 1,
    );

    if result.success {
        ddl_dir.record_installed(result.tool, &result.version, &result.method.to_string())?;
        output::print_install_result(true, result.tool, &result.message, args.is_json());
        let _ = crate::installer::run_tool_init(
            tool,
            args.verbose_quiet.raw_count() >= 1,
            args.yes || args.is_json(),
        );
        Ok(())
    } else {
        ddl_dir.record_failed(result.tool, &result.method.to_string())?;
        Err(DdlError::InstallFailed(result.message))
    }
}

pub fn cmd_install(tool_name: &str, args: &crate::cli::Args) -> Result<()> {
    let platform = detect_platform()?;
    let tool = crate::platform::resolve_tool(tool_name).map_err(DdlError::ToolNotFound)?;
    let mut ddl_dir = DdlDir::find_or_create()?;

    if already_installed(tool, &mut ddl_dir, args)? {
        return Ok(());
    }
    fresh_install(tool, &platform, &mut ddl_dir, args)
}
