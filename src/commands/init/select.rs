//! `ddl init` tool selection: flag parsing, retry semantics, interactive multiselect.

use super::*;

/// Whether the interactive selection was cancelled by the user.
pub enum Selection {
    Cancelled,
    Tools(Vec<String>),
}

/// Names parsed from an explicit `--tools a,b,c` flag.
fn parse_tools_flag(tools: &str) -> Vec<String> {
    // Empty entries from stray commas are dropped, not resolved as tool names
    tools
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

/// Tools eligible for (re)install: not yet on PATH, or whose previous
/// attempt failed (retry semantics).
fn retryable_tools(ddl_dir: &DdlDir) -> Vec<&'static str> {
    crate::platform::MANAGED_TOOLS
        .iter()
        .filter(|t| !crate::installer::is_tool_installed(t.name) || ddl_dir.should_retry(t.name))
        .map(|t| t.name)
        .collect()
}

/// Interactive multiselect over missing/failed tools (or all on first run).
/// Returns `Cancelled` when the user declines the confirmation.
/// Multiselect items: missing/failed tools on re-runs, all on first run.
fn selection_items(ddl_dir: &DdlDir) -> Vec<(&'static str, &'static str, &'static str)> {
    if !ddl_dir.manifest.tools.is_empty() {
        retryable_tools(ddl_dir)
            .into_iter()
            .map(|name| {
                let t = crate::platform::find_tool(name).expect("managed tool");
                (t.name, t.name, t.description)
            })
            .collect()
    } else {
        crate::platform::MANAGED_TOOLS
            .iter()
            .map(|t| (t.name, t.name, t.description))
            .collect()
    }
}

fn interactive_select(ddl_dir: &DdlDir, _args: &crate::cli::Args) -> Result<Selection> {
    let items = selection_items(ddl_dir);

    if items.is_empty() {
        // Everything is installed — fall through with an empty selection
        // so the tail work (forced wai init, skill checks) still runs.
        return Ok(Selection::Tools(Vec::new()));
    }

    let selected: Vec<&str> = cliclack::multiselect("Which tools would you like to install?")
        .items(&items)
        .interact()
        .map_err(|e| DdlError::Other(format!("Selection cancelled: {e}")))?;

    let confirmed = cliclack::confirm("Proceed with installation?")
        .interact()
        .map_err(|e| DdlError::Other(format!("Confirmation cancelled: {e}")))?;

    if !confirmed {
        return Ok(Selection::Cancelled);
    }

    Ok(Selection::Tools(
        selected.into_iter().map(String::from).collect(),
    ))
}

/// Determine which tools this init run covers.
pub fn select_tools(
    tools: Option<&str>,
    args: &crate::cli::Args,
    ddl_dir: &DdlDir,
) -> Result<Selection> {
    if let Some(tools_str) = tools {
        return Ok(Selection::Tools(parse_tools_flag(tools_str)));
    }
    if args.yes || args.is_json() {
        // Non-interactive: install all tools; already-initialized runs retry
        // failed tools and skip existing ones.
        let names = if ddl_dir.manifest.tools.is_empty() {
            crate::platform::MANAGED_TOOLS
                .iter()
                .map(|t| t.name.to_string())
                .collect()
        } else {
            retryable_tools(ddl_dir)
                .into_iter()
                .map(String::from)
                .collect()
        };
        return Ok(Selection::Tools(names));
    }
    interactive_select(ddl_dir, args)
}

/// DDL-6zn.8: every explicit --tools name must resolve — a typo (or a
/// self-name like 'dulce') must fail loudly instead of vanishing silently
/// from the install list.
pub fn validate_explicit_tools(tools: Option<&str>, selected: &[String]) -> Result<()> {
    if tools.is_none() {
        return Ok(());
    }
    let errors: Vec<String> = selected
        .iter()
        .filter_map(|name| crate::platform::resolve_tool(name).err())
        .collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(DdlError::ToolNotFound(errors.join("; ")))
    }
}
