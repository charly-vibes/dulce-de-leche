//! Tool installation orchestration: binary-first with cargo/npm fallbacks (DDL-ei3, DDL-5ph).

use super::*;
use crate::error::{DdlError, Result};
use crate::manifest::{Manifest, ToolEntry};
use crate::platform::{Os, PackageManager, Platform, Tool};
use std::path::PathBuf;
use std::process::Command;

/// Install a tool using the best available method.
/// Already-on-PATH fast path: report a Skipped install result.
fn install_skip_result(tool: &Tool) -> InstallResult {
    let ver = get_installed_version(tool.name).unwrap_or_else(|| "?".to_string());
    InstallResult {
        tool: tool.name,
        success: true,
        method: InstallMethod::Skipped,
        version: ver.clone(),
        message: format!("{} v{} is already installed", tool.name, ver),
    }
}

/// Binary install with the cargo fallback when the platform has no release
/// binary. Definitive miss (no release asset for this platform) falls back
/// to cargo when available — but only for tools with a real crates.io
/// package (Go/npm tools have none, and installing the wrong crate would be
/// actively harmful).
fn install_binary_with_fallback(
    tool: &Tool,
    platform: &Platform,
    verbose: bool,
) -> (InstallMethod, Result<VerificationStatus>) {
    match install_binary(tool, platform, verbose) {
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
                        install_cargo(tool, verbose).map(|()| VerificationStatus::Unverified),
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
    }
}

/// Record a successful install in the manifest and build the result.
fn install_success_result(
    tool: &Tool,
    method: &InstallMethod,
    detected: &str,
    status: VerificationStatus,
    manifest: &mut Manifest,
) -> InstallResult {
    manifest.set_tool(
        tool.name,
        ToolEntry {
            installed: detected.to_string(),
            source: method.to_string(),
            status: "installed".to_string(),
            compatible: ">=0.0.0".to_string(),
        },
    );
    InstallResult {
        tool: tool.name,
        success: true,
        method: method.clone(),
        version: detected.to_string(),
        message: format!(
            "{} v{} installed via {}{}",
            tool.name,
            detected,
            method,
            status.suffix()
        ),
    }
}

/// Record a failed install in the manifest and build the failure result.
fn install_failure_result(
    tool: &Tool,
    method: &InstallMethod,
    error: &DdlError,
    manifest: &mut Manifest,
) -> InstallResult {
    manifest.set_tool(
        tool.name,
        ToolEntry {
            installed: "unknown".to_string(),
            source: method.to_string(),
            status: "failed".to_string(),
            compatible: ">=0.0.0".to_string(),
        },
    );
    InstallResult {
        tool: tool.name,
        success: false,
        method: method.clone(),
        version: String::new(),
        message: format!("{} failed: {}", tool.name, error),
    }
}

pub fn install_tool(
    tool: &Tool,
    platform: &Platform,
    manifest: &mut Manifest,
    verbose: bool,
) -> InstallResult {
    let method = best_install_method(tool, platform);

    if is_tool_installed(tool.name) {
        return install_skip_result(tool);
    }

    let (method, result) = match method {
        InstallMethod::Binary => install_binary_with_fallback(tool, platform, verbose),
        other => {
            let result = execute_install(&other, tool, platform, verbose)
                .map(|()| VerificationStatus::Unverified);
            (other, result)
        }
    };

    let detected = resolve_with_dest(tool.name, Some(&binary_dest_path(tool, platform)))
        .unwrap_or_else(|| "unknown".to_string());

    match result {
        Ok(status) => install_success_result(tool, &method, &detected, status, manifest),
        Err(e) => install_failure_result(tool, &method, &e, manifest),
    }
}

/// Dispatch a single install attempt for the given method.
pub(crate) fn execute_install(
    method: &InstallMethod,
    tool: &Tool,
    platform: &Platform,
    verbose: bool,
) -> Result<()> {
    match method {
        // Binary status flows through the install_tool first-arm path; this
        // dispatch arm is only reached for Cargo/Npm, so the status is
        // irrelevant here.
        InstallMethod::Binary => install_binary(tool, platform, verbose).map(|_| ()),
        InstallMethod::Cargo => install_cargo(tool, verbose),
        InstallMethod::Npm => install_npm(tool, verbose),
        InstallMethod::Skipped => unreachable!(),
    }
}

/// Dispatch a single upgrade attempt for the given method.
pub(crate) fn execute_upgrade(
    method: &InstallMethod,
    tool: &Tool,
    platform: &Platform,
    verbose: bool,
) -> Result<()> {
    match method {
        InstallMethod::Binary => upgrade_binary(tool, platform, verbose).map(|_| ()),
        InstallMethod::Cargo => upgrade_cargo(tool, verbose),
        InstallMethod::Npm => upgrade_npm(tool, verbose),
        InstallMethod::Skipped => unreachable!(),
    }
}

/// Install a tool via binary download from GitHub releases.
/// Returns the download's verification status (DDL-yts) on success.
pub(crate) fn install_binary(
    tool: &Tool,
    platform: &Platform,
    verbose: bool,
) -> Result<VerificationStatus> {
    let binary_name = if platform.os == Os::Windows {
        std::path::PathBuf::from(tool.name)
            .with_extension("exe")
            .to_string_lossy()
            .to_string()
    } else {
        tool.name.to_string()
    };

    let target = match (platform.os.as_str(), platform.arch.as_str()) {
        ("darwin", "arm64") => "darwin_arm64",
        ("darwin", "amd64") => "darwin_amd64",
        ("linux", "arm64") => "linux_arm64",
        ("linux", "amd64") => "linux_amd64",
        ("windows", "amd64") => "windows_amd64",
        _ => return Err(DdlError::UnsupportedPlatform(platform.as_str())),
    };

    let client = github_client()?;

    let releases_url = format!("https://api.github.com/repos/{}/releases/latest", tool.repo);
    let resp = client
        .get(&releases_url)
        .send()
        .map_err(DdlError::Network)?;

    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(DdlError::NoReleaseBinary {
            tool: tool.name.to_string(),
            url: releases_url,
        });
    }
    if resp.status() == reqwest::StatusCode::FORBIDDEN {
        return Err(DdlError::InstallFailed(format!(
            "GitHub API rate limit reached for {releases_url}. \
             Set GITHUB_TOKEN (a PAT with public repo read access) to raise the limit."
        )));
    }
    if !resp.status().is_success() {
        return Err(DdlError::InstallFailed(format!(
            "GitHub API returned {} for {}",
            resp.status(),
            releases_url
        )));
    }

    let release: serde_json::Value = resp.json().map_err(DdlError::Network)?;

    let tag = release["tag_name"]
        .as_str()
        .ok_or_else(|| DdlError::InstallFailed("No tag_name in release".to_string()))?;
    let version = tag.trim_start_matches('v');

    let ext = if platform.os == Os::Windows {
        "zip"
    } else {
        "tar.gz"
    };
    // Asset prefix may differ from the binary name (e.g. beads ships
    // `beads_<ver>_<target>` archives containing the `bd` binary).
    let asset_prefix = tool.binary_asset_prefix();
    let archive_name = format!("{asset_prefix}_{version}_{target}");
    let download_url = format!(
        "https://github.com/{}/releases/download/{}/{}.{}",
        tool.repo, tag, archive_name, ext
    );

    verbose_print(
        verbose,
        &format!("downloading {}/releases/latest for {}", tool.repo, target),
    );

    // Determine install destination
    let dest_dir = if platform.os == Os::Windows {
        let local_app_data = std::env::var("LOCALAPPDATA").map_err(|_| {
            DdlError::PrerequisiteMissing(
                "%LOCALAPPDATA% not set. On Windows, this should point to AppData\\Local."
                    .to_string(),
            )
        })?;
        binary_dest_dir(
            &platform.os,
            None,
            Some(PathBuf::from(local_app_data).as_path()),
        )
    } else {
        binary_dest_dir(&platform.os, dirs::home_dir().as_deref(), None)
    };

    std::fs::create_dir_all(&dest_dir).map_err(DdlError::Io)?;

    let dest_path = dest_dir.join(&binary_name);

    // Verification runs BEFORE any destination mutation: the archive bytes are
    // only extracted once the checksum policy has accepted them (DDL-yts).
    // DDL-1z1: budgets are exposed in diagnostics so the limits a download,
    // extraction or child call is held to are visible, not implicit.
    verbose_print(
        verbose,
        &format!(
            "budgets: download ≤ {MAX_RELEASE_ASSET_BYTES} bytes; extracted binary ≤ {MAX_BINARY_BYTES} bytes; child runtime ≤ {} s; captured output ≤ {MAX_CHILD_OUTPUT_BYTES} bytes",
            child_deadline().as_secs()
        ),
    );
    let archive_bytes = fetch_release_asset(&download_url, tool.name)?;
    let status = verify_download(&download_url, &archive_bytes, tool, verbose)?;
    match status {
        VerificationStatus::Verified => {
            eprintln!("  ✓ {} sha256 verified against release checksum", tool.name)
        }
        VerificationStatus::ChecksumsVerified => eprintln!(
            "  ✓ {} sha256 verified against the release's own checksums (external origin — no provenance manifest required)",
            tool.name
        ),
        VerificationStatus::UnverifiedAuthorized => eprintln!(
            "  ⚠ {} installed WITHOUT checksum verification ({}): explicitly authorized via DDL_ALLOW_UNVERIFIED_RELEASE",
            tool.name, download_url
        ),
        VerificationStatus::Unverified => {}
    }
    crate::output::record_install_verification(tool.name, status.label());

    stage_and_replace_binary(&archive_bytes, &dest_path, tool.name, &download_url)?;

    ensure_on_path(&dest_dir, platform)?;

    eprintln!("  ✓ {} downloaded to {}", tool.name, dest_path.display());
    Ok(status)
}

/// SHA-256 of `bytes` as lowercase hex.
pub(crate) fn install_cargo(tool: &Tool, verbose: bool) -> Result<()> {
    verbose_print(
        verbose,
        &format!("running: cargo install {}", tool.crate_name),
    );
    let mut command = Command::new("cargo");
    apply_child_policy(&mut command, ExecutionScope::PackageBuild, &[]).map_err(|e| {
        DdlError::InstallFailed(format!("cargo install rejected by child policy: {e}"))
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

/// Install a tool via npm (global).
pub(crate) fn install_npm(tool: &Tool, verbose: bool) -> Result<()> {
    // The npm package may be scoped (e.g. @fission-ai/openspec) — always use
    // npm_package, falling back to crate_name for registry-style entries.
    let package = tool.npm_package.unwrap_or(tool.crate_name);
    verbose_print(verbose, &format!("running: npm install -g {package}"));
    let mut command = npm_command("npm");
    apply_child_policy(&mut command, ExecutionScope::PackageBuild, &[]).map_err(|e| {
        DdlError::InstallFailed(format!("npm install rejected by child policy: {e}"))
    })?;
    let status = command
        .args(["install", "-g", package])
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status()
        .map_err(|e| DdlError::InstallFailed(format!("Failed to run npm: {e}")))?;

    if status.success() {
        Ok(())
    } else {
        Err(DdlError::InstallFailed(format!(
            "npm install -g {package} exited with code {}",
            status.code().unwrap_or(-1)
        )))
    }
}
