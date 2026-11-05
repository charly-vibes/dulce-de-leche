//! `ddl version` — local versions, optional latest-version check, JSON and
//! human output.

use super::*;

/// Build the JSON entry for one manifest-tracked tool.
fn json_tool_entry(
    name: &str,
    entry: &crate::manifest::ToolEntry,
    matrix: &crate::compat::CompatibilityMatrix,
    latest_versions: &std::collections::HashMap<&'static str, String>,
) -> serde_json::Value {
    let constraint = matrix.constraint(name).unwrap_or("*");
    let compatible = matrix.is_compatible(name, &entry.installed);
    let latest = latest_versions.get(name);
    let update_available = latest.is_some_and(|lv| *lv != entry.installed);
    let registry_tool = crate::platform::find_tool(name);
    serde_json::json!({
        "name": name,
        "version": entry.installed,
        "latest": latest,
        "update_available": update_available,
        "source": entry.source,
        "status": entry.status,
        "compatible": compatible,
        "constraint": constraint,
        "category": registry_tool.map(|t| t.category.as_str()),
        "maturity": registry_tool.map(|t| t.maturity.as_str())
    })
}

/// JSON envelope: manifest versions plus (with --check) latest versions.
fn version_json(check: bool) -> Result<()> {
    let latest_versions = if check {
        crate::installer::check_all_latest_versions()
    } else {
        std::collections::HashMap::new()
    };

    let mut tools: Vec<serde_json::Value> = Vec::new();
    if let Some(ddl_dir) = DdlDir::find() {
        let (matrix, _warnings) = crate::compat::load_compatibility(&ddl_dir.path);
        let mut names: Vec<&String> = ddl_dir.manifest.tools.keys().collect();
        names.sort();
        for name in names {
            if let Some(entry) = ddl_dir.manifest.tools.get(name) {
                tools.push(json_tool_entry(name, entry, &matrix, &latest_versions));
            }
        }
    }
    let data = serde_json::json!({
        "ddl_version": crate::VERSION,
        "check": check,
        "tools": tools
    });
    let json_str = output::json_output(
        crate::VERSION,
        true,
        EnvelopeKind::Version,
        data,
        vec![],
        vec![],
    )?;
    println!("{json_str}");

    if check && tools.iter().any(|t| t["update_available"] == true) {
        return Err(DdlError::PartialFailure);
    }
    Ok(())
}

/// Human listing: manifest-tracked tools when a .ddl/ exists, otherwise probe
/// every managed tool on PATH.
fn human_listing() {
    println!("ddl: {}", crate::VERSION);
    println!();

    match DdlDir::find() {
        Some(ddl_dir) => {
            let mut names: Vec<&String> = ddl_dir.manifest.tools.keys().collect();
            names.sort();
            for name in names {
                if let Some(entry) = ddl_dir.manifest.tools.get(name) {
                    println!("  {:12} v{} (via {})", name, entry.installed, entry.source);
                }
            }
        }
        None => {
            use crate::diagnostics;
            for tool in crate::platform::MANAGED_TOOLS {
                if diagnostics::which(tool.name).is_some() {
                    let version =
                        diagnostics::get_tool_version(tool).unwrap_or_else(|| "?".to_string());
                    println!(
                        "  {:12} v{} [{}/{}]",
                        tool.name, version, tool.category, tool.maturity
                    );
                }
            }
        }
    }
}

/// Update-status line for one managed tool in the human `--check` table.
/// Returns (installed, version, status).
fn check_tool_status(
    tool: &crate::platform::Tool,
    latest_versions: &std::collections::HashMap<&'static str, String>,
    network_ok: bool,
    any_outdated: &mut bool,
) -> (bool, String, String) {
    let installed = crate::installer::is_tool_installed(tool.name);
    let version = if installed {
        crate::installer::get_installed_version(tool.name).unwrap_or_else(|| "?".to_string())
    } else {
        "not installed".to_string()
    };

    let latest = latest_versions.get(tool.name);
    let status = if !installed {
        "".to_string()
    } else if let Some(lv) = latest {
        if *lv != version {
            *any_outdated = true;
            format!("update available: v{lv}")
        } else {
            "up to date".to_string()
        }
    } else if network_ok {
        // Network worked but no version found for this tool
        "".to_string()
    } else {
        "".to_string()
    };
    (installed, version, status)
}

/// Human `--check` pass: query latest versions and print per-tool status.
/// Print the per-tool version/constraint table for the human `--check` pass.
fn print_check_table(
    latest_versions: &std::collections::HashMap<&'static str, String>,
    network_ok: bool,
) -> bool {
    let mut any_outdated = false;
    let matrix = if let Some(ddl_dir) = DdlDir::find() {
        let (matrix, _warnings) = crate::compat::load_compatibility(&ddl_dir.path);
        matrix
    } else {
        crate::compat::CompatibilityMatrix::embedded()
    };

    for tool in crate::platform::MANAGED_TOOLS {
        let (installed, version, status) =
            check_tool_status(tool, latest_versions, network_ok, &mut any_outdated);
        let constraint = matrix.constraint(tool.name).unwrap_or("*");

        if installed {
            println!(
                "  {:12} v{} [{}/{}] (constraint: {}) {}",
                tool.name, version, tool.category, tool.maturity, constraint, status
            );
        } else {
            println!(
                "  {:12} {} [{}/{}] (constraint: {})",
                tool.name, version, tool.category, tool.maturity, constraint
            );
        }
    }
    any_outdated
}

fn check_updates_human() -> Result<()> {
    println!();
    println!("Checking for updates... (requires network)");
    println!();

    let latest_versions = crate::installer::check_all_latest_versions();
    let network_ok = !latest_versions.is_empty();

    if !network_ok {
        println!("  ⚠ Network unavailable — showing cached/embedded versions");
        println!();
    }

    if print_check_table(&latest_versions, network_ok) {
        println!();
        println!("Some tools have updates available. Run `ddl upgrade` to update.");
        return Err(DdlError::PartialFailure);
    }
    Ok(())
}

pub fn cmd_version(check: bool, args: &crate::cli::Args) -> Result<()> {
    if args.is_json() {
        return version_json(check);
    }
    human_listing();
    if check {
        check_updates_human()?;
    }
    Ok(())
}
