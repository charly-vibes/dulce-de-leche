//! `ddl migrate` — phase-1 config migration under .ddl/ and its undo.

use super::*;

/// Bulk undo: legacy paths are restored as copies of the .ddl/ backing store,
/// which is left untouched (DDL-6zn.7). Clears phase1 residue (gh#30).
fn migrate_undo(ddl_dir: DdlDir, args: &crate::cli::Args) -> Result<()> {
    let restored = ddl_dir.unmigrate_all()?;
    if restored.is_empty() {
        output::print_success("No migrated configs found.", args.is_json());
        return Ok(());
    }
    for (tool_name, legacy_path) in &restored {
        if !args.is_json() {
            println!("  Restoring {}...", tool_name);
        }
        output::print_install_result(
            true,
            tool_name,
            &format!("{} restored to {}", tool_name, legacy_path.display()),
            args.is_json(),
        );
    }
    // Clear the migration residue: the manifest must not keep claiming
    // phase1 once every symlink has been restored (gh#30).
    let mut ddl_dir = ddl_dir;
    ddl_dir.manifest.migration_state = "none".to_string();
    ddl_dir.save_manifest()?;
    Ok(())
}

/// Migrate one legacy config unless its path is git-tracked (safety guard,
/// gh#30: git does not follow symlinks, so replacing a tracked path with one
/// makes every tracked file under it vanish from the repo). Returns true
/// when the tool was skipped because it is tracked.
fn migrate_one(
    ddl_dir: &mut DdlDir,
    tool_name: &str,
    legacy_path: &std::path::Path,
    args: &crate::cli::Args,
) -> Result<bool> {
    if crate::dot_ddl::is_git_tracked(legacy_path) {
        if !args.is_json() {
            println!(
                "  Skipping {}: {} is git-tracked — symlinking it would break the repo for other clones. To migrate anyway, untrack it first (git rm -r --cached {}) and commit.",
                tool_name,
                legacy_path.display(),
                legacy_path.display()
            );
        }
        return Ok(true);
    }
    if !args.is_json() {
        println!(
            "  Migrating {} from {}...",
            tool_name,
            legacy_path.display()
        );
    }
    ddl_dir.migrate_tool(tool_name, legacy_path)?;
    output::print_install_result(
        true,
        tool_name,
        &format!(
            "{} migrated to {}",
            tool_name,
            ddl_dir.tool_path(tool_name).display()
        ),
        args.is_json(),
    );
    Ok(false)
}

/// Forward migration: move legacy configs under .ddl/ and stamp phase1.
fn migrate_forward(ddl_dir: DdlDir, args: &crate::cli::Args) -> Result<()> {
    let legacy_configs = ddl_dir.detect_legacy_configs();
    if legacy_configs.is_empty() {
        output::print_success(
            "No legacy configs found. Nothing to migrate.",
            args.is_json(),
        );
        return Ok(());
    }

    if !args.is_json() {
        println!("Phase 1 migration — moving configs under .ddl/...");
    }

    let mut skipped_tracked = Vec::new();
    let mut ddl_dir = ddl_dir;
    for (tool_name, legacy_path) in &legacy_configs {
        if migrate_one(&mut ddl_dir, tool_name, legacy_path, args)? {
            skipped_tracked.push((*tool_name, legacy_path.clone()));
        }
    }

    if skipped_tracked.len() == legacy_configs.len() {
        output::print_success(
            "All legacy configs are git-tracked — nothing migrated. Untrack them first if you want them under .ddl/.",
            args.is_json(),
        );
        return Ok(());
    }

    ddl_dir.manifest.migration_state = "phase1".to_string();
    ddl_dir.save_manifest()?;

    if !args.is_json() {
        println!();
        println!("Done. All configs are now under .ddl/.");
        println!("Run `ddl migrate --undo` to restore the previous layout.");
    }

    Ok(())
}

pub fn cmd_migrate(undo: bool, args: &crate::cli::Args) -> Result<()> {
    let ddl_dir = DdlDir::find_or_create()?;
    if undo {
        migrate_undo(ddl_dir, args)
    } else {
        migrate_forward(ddl_dir, args)
    }
}
