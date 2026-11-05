//! Installed-tool probing: version discovery, dest paths, GitHub client.

use super::*;
use crate::error::{DdlError, Result};
use crate::platform::{Os, Platform, Tool};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Check if a tool is already installed on PATH.
pub fn is_tool_installed(name: &str) -> bool {
    which(name).is_some()
}

/// Get the version of an installed tool.
pub fn get_installed_version(name: &str) -> Option<String> {
    // npm packages don't support `--version` uniformly — ask npm instead.
    // Use the npm package name, which may be scoped and differ from the
    // binary name (e.g. binary `openspec`, package `@fission-ai/openspec`).
    if let Some(tool) = crate::platform::find_tool(name)
        && tool.npm_package.is_some()
    {
        return get_npm_global_version(tool.npm_package.unwrap_or(name));
    }
    // DDL-1z1: even a version probe is budgeted — a hung or flooding child
    // cannot stall bootstrap past the deadline.
    let output = run_child_captured(Command::new(name).arg("--version"), child_deadline()).ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let first_line = stdout.lines().next()?;
    parse_version_line(name, first_line)
}

/// Parse a `--version` first line into a version string.
///
/// `name` is the binary name; when the first whitespace token equals it
/// (e.g. `bd version 1.3.0`), the version is taken from the second token.
pub(crate) fn parse_version_line(name: &str, first_line: &str) -> Option<String> {
    let parts: Vec<&str> = first_line.split_whitespace().collect();
    if parts.len() >= 2 {
        let candidate = if parts[0].to_lowercase() == name {
            parts[1]
        } else {
            parts[0]
        };
        let version = if candidate.to_lowercase() == "version" && parts.len() >= 3 {
            parts[2]
        } else {
            candidate
        };
        Some(version.trim_start_matches('v').to_string())
    } else {
        Some(first_line.trim_start_matches('v').to_string())
    }
}

/// Probe a binary directly at `path` (e.g. a just-downloaded `.ddl/bin`
/// binary that is not yet on PATH) and parse its `--version` output.
/// Fixes DDL-x0m: installs recorded `unknown` when `.ddl/bin` was not on
/// the current process's PATH at probe time.
pub(crate) fn probe_version_at(path: &Path, name: &str) -> Option<String> {
    if !path.exists() {
        return None;
    }
    let output = run_child_captured(Command::new(path).arg("--version"), child_deadline()).ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let first_line = stdout.lines().next()?;
    parse_version_line(name, first_line)
}

/// Destination directory for binary installs. Pure so tests can inject the
/// home / LOCALAPPDATA roots.
pub(crate) fn binary_dest_dir(
    os: &Os,
    home: Option<&Path>,
    local_app_data: Option<&Path>,
) -> PathBuf {
    match os {
        Os::Windows => local_app_data
            .map(PathBuf::from)
            .unwrap_or_default()
            .join("ddl")
            .join("bin"),
        _ => home.map(|h| h.join(".ddl").join("bin")).unwrap_or_default(),
    }
}

/// Absolute path of the binary that `install_binary` would write for `tool`.
pub(crate) fn binary_dest_path(tool: &Tool, platform: &Platform) -> PathBuf {
    let binary_name = if platform.os == Os::Windows {
        std::path::PathBuf::from(tool.name)
            .with_extension("exe")
            .to_string_lossy()
            .to_string()
    } else {
        tool.name.to_string()
    };
    let dest_dir = if platform.os == Os::Windows {
        let local_app_data = std::env::var("LOCALAPPDATA").ok().map(PathBuf::from);
        binary_dest_dir(&platform.os, None, local_app_data.as_deref())
    } else {
        binary_dest_dir(&platform.os, dirs::home_dir().as_deref(), None)
    };
    dest_dir.join(binary_name)
}

/// The ddl-managed binary directory (`~/.ddl/bin`, or the Windows
/// LOCALAPPDATA equivalent) — prepended to child PATH during init cascades
/// so freshly downloaded tools can initialize and find their siblings.
pub fn ddl_bin_dir() -> Option<PathBuf> {
    let platform = Platform::detect()?;
    Some(if platform.os == Os::Windows {
        let local_app_data = std::env::var("LOCALAPPDATA").ok().map(PathBuf::from);
        binary_dest_dir(&platform.os, None, local_app_data.as_deref())
    } else {
        binary_dest_dir(&platform.os, dirs::home_dir().as_deref(), None)
    })
}

/// Resolve the installed version of a binary tool: PATH probe first, then a
/// direct probe at the `.ddl/bin` destination (covers the window between
/// download and the next shell picking up the PATH entry — DDL-x0m).
pub(crate) fn resolve_with_dest(binary_name: &str, dest_dir: Option<&Path>) -> Option<String> {
    get_installed_version(binary_name)
        .or_else(|| dest_dir.and_then(|d| probe_version_at(&d.join(binary_name), binary_name)))
}

/// Get the globally installed npm version of a package via `npm list -g`.
pub(crate) fn get_npm_global_version(package: &str) -> Option<String> {
    let output = run_child_captured(
        npm_command("npm").args(["list", "-g", package, "--depth=0"]),
        child_deadline(),
    )
    .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .find(|l| l.contains(&format!("{package}@")))
        .and_then(|l| l.rsplit('@').next())
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

/// GitHub access token for API requests, if available.
///
/// Reads `GITHUB_TOKEN` then `GH_TOKEN` (both conventions are common).
/// Unauthenticated api.github.com requests are limited to 60/hour per IP;
/// shared CI runners routinely exhaust that quota, so ddl attaches the
/// token whenever the environment provides one (Actions runners always do).
pub(crate) fn github_token() -> Option<String> {
    ["GITHUB_TOKEN", "GH_TOKEN"]
        .iter()
        .find_map(|var| std::env::var(var).ok().filter(|t| !t.is_empty()))
}

/// Build an HTTP client for GitHub API calls: ddl user-agent plus a Bearer
/// token when one is available (see `github_token`).
pub(crate) fn github_client() -> Result<reqwest::blocking::Client> {
    let mut headers = reqwest::header::HeaderMap::new();
    if let Some(token) = github_token() {
        let value = reqwest::header::HeaderValue::from_str(&format!("Bearer {token}"))
            .map_err(|e| DdlError::Other(format!("Invalid GitHub token: {e}")))?;
        headers.insert(reqwest::header::AUTHORIZATION, value);
    }
    reqwest::blocking::Client::builder()
        .user_agent(format!("ddl/{}", env!("CARGO_PKG_VERSION")))
        .default_headers(headers)
        .build()
        .map_err(|e| DdlError::Other(format!("Failed to create HTTP client: {e}")))
}
