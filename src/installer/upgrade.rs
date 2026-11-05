//! Tool upgrade orchestration and latest-version checks (DDL-5bs replay policy).

use super::*;
use crate::error::{DdlError, Result};
use crate::manifest::{Manifest, ToolEntry};
use crate::platform::{MANAGED_TOOLS, PackageManager, Platform, Tool};
use std::process::Command;

/// Upgrade a tool using the method recorded in the manifest.
pub fn upgrade_tool(
    tool: &Tool,
    platform: &Platform,
    manifest: &mut Manifest,
    verbose: bool,
) -> InstallResult {
    // Determine the upgrade method based on the manifest's recorded source.
    // Keeping the recorded channel avoids PATH shadowing (e.g. re-routing a
    // cargo-installed tool to a binary download would leave two copies on
    // PATH). Legacy brew/scoop records upgrade via the binary-first chain.
    let method = match manifest.get_tool(tool.name) {
        Some(entry) => match entry.source.as_str() {
            "cargo install" => InstallMethod::Cargo,
            "npm install" => InstallMethod::Npm,
            _ => InstallMethod::Binary,
        },
        // Not in manifest — use the standard policy
        None => best_install_method(tool, platform),
    };

    let old_version = get_installed_version(tool.name);

    let (method, result) = match method {
        InstallMethod::Binary => match upgrade_binary(tool, platform, verbose) {
            Ok(status) => (InstallMethod::Binary, Ok(status)),
            Err(DdlError::NoReleaseBinary { .. }) => {
                match fallback_after_binary_failure(
                    tool.cargo_installable() && PackageManager::Cargo.is_available(),
                ) {
                    Some(InstallMethod::Cargo) => {
                        verbose_print(
                            verbose,
                            &format!(
                                "⚠ {} binary not yet available for this platform — using cargo install instead",
                                tool.name
                            ),
                        );
                        (
                            InstallMethod::Cargo,
                            upgrade_cargo(tool, verbose).map(|()| VerificationStatus::Unverified),
                        )
                    }
                    _ => (
                        InstallMethod::Binary,
                        Err(DdlError::InstallFailed(format!(
                            "⚠ {} binary not yet available for this platform, and no cargo fallback is available. \
                             Download the binary manually from \
                             https://github.com/{}/releases",
                            tool.name, tool.repo
                        ))),
                    ),
                }
            }
            Err(e) => (InstallMethod::Binary, Err(e)),
        },
        other => {
            let result = execute_upgrade(&other, tool, platform, verbose)
                .map(|()| VerificationStatus::Unverified);
            (other, result)
        }
    };

    let new_version = resolve_with_dest(tool.name, Some(&binary_dest_path(tool, platform)));

    match result {
        Ok(status) => {
            let ver = new_version.clone().unwrap_or_else(|| "unknown".to_string());
            let was_upgraded = match (&old_version, &new_version) {
                (Some(old), Some(new)) => old != new,
                _ => true,
            };

            manifest.set_tool(
                tool.name,
                ToolEntry {
                    installed: ver.clone(),
                    source: method.to_string(),
                    status: "installed".to_string(),
                    compatible: ">=0.0.0".to_string(),
                },
            );

            if was_upgraded {
                InstallResult {
                    tool: tool.name,
                    success: true,
                    method: method.clone(),
                    version: ver.clone(),
                    message: format!(
                        "{} upgraded to v{} via {}{}",
                        tool.name,
                        ver,
                        method,
                        status.suffix()
                    ),
                }
            } else {
                InstallResult {
                    tool: tool.name,
                    success: true,
                    method: InstallMethod::Skipped,
                    version: ver.clone(),
                    message: format!(
                        "{} v{} is already up to date{}",
                        tool.name,
                        ver,
                        status.suffix()
                    ),
                }
            }
        }
        Err(e) => {
            manifest.set_tool(
                tool.name,
                ToolEntry {
                    installed: old_version.unwrap_or_else(|| "unknown".to_string()),
                    source: method.to_string(),
                    status: "failed".to_string(),
                    compatible: ">=0.0.0".to_string(),
                },
            );
            InstallResult {
                tool: tool.name,
                success: false,
                method,
                version: String::new(),
                message: format!("{} upgrade failed: {}", tool.name, e),
            }
        }
    }
}

/// Upgrade a tool installed via cargo.
pub(crate) fn upgrade_cargo(tool: &Tool, verbose: bool) -> Result<()> {
    verbose_print(
        verbose,
        &format!("running: cargo install {}", tool.crate_name),
    );
    // `cargo install` is idempotent — it upgrades if already installed
    let mut command = Command::new("cargo");
    apply_child_policy(&mut command, ExecutionScope::PackageBuild, &[]).map_err(|e| {
        DdlError::InstallFailed(format!("cargo upgrade rejected by child policy: {e}"))
    })?;
    let status = command
        .args(["install", tool.crate_name])
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status()
        .map_err(|e| DdlError::InstallFailed(format!("Failed to run cargo: {e}")))?;

    if status.success() {
        Ok(())
    } else {
        Err(DdlError::InstallFailed(format!(
            "cargo install {} exited with code {}",
            tool.crate_name,
            status.code().unwrap_or(-1)
        )))
    }
}

/// Upgrade a tool installed via npm (reinstall at latest).
pub(crate) fn upgrade_npm(tool: &Tool, verbose: bool) -> Result<()> {
    // Scoped packages (e.g. @fission-ai/openspec) — use npm_package.
    let package = tool.npm_package.unwrap_or(tool.crate_name);
    verbose_print(
        verbose,
        &format!("running: npm install -g {package}@latest"),
    );
    let mut command = npm_command("npm");
    apply_child_policy(&mut command, ExecutionScope::PackageBuild, &[]).map_err(|e| {
        DdlError::InstallFailed(format!("npm upgrade rejected by child policy: {e}"))
    })?;
    let status = command
        .args(["install", "-g", &format!("{package}@latest")])
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status()
        .map_err(|e| DdlError::InstallFailed(format!("Failed to run npm: {e}")))?;

    if status.success() {
        Ok(())
    } else {
        Err(DdlError::InstallFailed(format!(
            "npm install -g {package}@latest exited with code {}",
            status.code().unwrap_or(-1)
        )))
    }
}

/// Upgrade a tool installed via binary download (re-download).
pub(crate) fn upgrade_binary(
    tool: &Tool,
    platform: &Platform,
    verbose: bool,
) -> Result<VerificationStatus> {
    // Re-download the binary — same as install_binary but without the
    // is_tool_installed check (which is handled by the caller)
    install_binary(tool, platform, verbose)
}

/// Upgrade all tools recorded in the manifest.
pub fn upgrade_all_tools(
    platform: &Platform,
    manifest: &mut Manifest,
    verbose: bool,
) -> Vec<InstallResult> {
    let tool_names: Vec<String> = manifest.tools.keys().cloned().collect();
    tool_names
        .iter()
        .filter_map(|name| {
            let tool = crate::platform::find_tool(name)?;
            Some(upgrade_tool(tool, platform, manifest, verbose))
        })
        .collect()
}

/// Upgrade a subset of tools by name.
pub fn upgrade_selected_tools(
    names: &[String],
    platform: &Platform,
    manifest: &mut Manifest,
    verbose: bool,
) -> Vec<InstallResult> {
    names
        .iter()
        .filter_map(|name| {
            let tool = crate::platform::find_tool(name)?;
            Some(upgrade_tool(tool, platform, manifest, verbose))
        })
        .collect()
}

/// Build a Command for an npm-ecosystem executable (npm, npx, or a
/// package-installed binary like `incitaciones`).
///
/// On Windows these are `.cmd` shims, which Rust's std deliberately does not
/// execute via CreateProcess (BatBadBut mitigation, CVE-2024-24576) — so we
/// route through `cmd /C` there. On other platforms, exec directly.
pub(crate) fn npm_command(program: &str) -> Command {
    #[cfg(windows)]
    {
        let mut cmd = Command::new("cmd");
        cmd.args(["/C", program]);
        cmd
    }
    #[cfg(not(windows))]
    {
        Command::new(program)
    }
}

// ==========================================================================
// DDL-uu8 (SEC-04): restricted-privilege policy for spawned children
// ==========================================================================

/// Authorization variable for the global skill-install scope
/// (`incitaciones install --global` writes to `~/.agents/skills/`, which is
/// visible to every project on the machine).
/// Check the latest available version of a tool from its GitHub releases.
pub fn check_latest_version(tool: &Tool) -> Option<String> {
    // npm packages: query the npm registry via the npm CLI.
    if tool.npm_package.is_some() {
        let output = npm_command("npm")
            .args([
                "view",
                tool.npm_package.unwrap_or(tool.crate_name),
                "version",
            ])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let version = stdout.trim();
        return if version.is_empty() {
            None
        } else {
            Some(version.to_string())
        };
    }

    let client = github_client().ok()?;

    let releases_url = format!("https://api.github.com/repos/{}/releases/latest", tool.repo);
    let resp = client.get(&releases_url).send().ok()?;

    if !resp.status().is_success() {
        return None;
    }

    let release: serde_json::Value = resp.json().ok()?;
    let tag = release["tag_name"].as_str()?;
    Some(tag.trim_start_matches('v').to_string())
}

/// Check the latest versions of all managed tools.
/// Returns a map of tool name to latest version.
pub fn check_all_latest_versions() -> std::collections::HashMap<&'static str, String> {
    let mut versions = std::collections::HashMap::new();
    for tool in MANAGED_TOOLS {
        if let Some(ver) = check_latest_version(tool) {
            versions.insert(tool.name, ver);
        }
    }
    versions
}

/// Install all tools, returning results for each.
pub fn install_all_tools(
    platform: &Platform,
    manifest: &mut Manifest,
    verbose: bool,
) -> Vec<InstallResult> {
    MANAGED_TOOLS
        .iter()
        .map(|tool| install_tool(tool, platform, manifest, verbose))
        .collect()
}

/// Install a subset of tools by name.
pub fn install_selected_tools(
    names: &[String],
    platform: &Platform,
    manifest: &mut Manifest,
    verbose: bool,
) -> Vec<InstallResult> {
    names
        .iter()
        .filter_map(|name| {
            let tool = crate::platform::find_tool(name)?;
            Some(install_tool(tool, platform, manifest, verbose))
        })
        .collect()
}
