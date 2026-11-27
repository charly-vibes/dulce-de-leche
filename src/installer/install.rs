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
/// Map the (os, arch) pair to the release asset target slug.
fn binary_asset_target(platform: &Platform) -> Result<&'static str> {
    match (platform.os.as_str(), platform.arch.as_str()) {
        ("darwin", "arm64") => Ok("darwin_arm64"),
        ("darwin", "amd64") => Ok("darwin_amd64"),
        ("linux", "arm64") => Ok("linux_arm64"),
        ("linux", "amd64") => Ok("linux_amd64"),
        ("windows", "amd64") => Ok("windows_amd64"),
        _ => Err(DdlError::UnsupportedPlatform(platform.as_str())),
    }
}

/// Fetch the latest release metadata from GitHub, mapping 404 to the
/// NoReleaseBinary fallback signal and 403 to a rate-limit remedy message.
fn fetch_latest_release(
    client: &reqwest::blocking::Client,
    tool: &Tool,
) -> Result<serde_json::Value> {
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

    resp.json().map_err(DdlError::Network)
}

/// Build the archive download URL for the platform target.
fn binary_download_url(
    tool: &Tool,
    tag: &str,
    version: &str,
    target: &str,
    platform: &Platform,
) -> String {
    let ext = if platform.os == Os::Windows {
        "zip"
    } else {
        "tar.gz"
    };
    // Asset prefix may differ from the binary name (e.g. beads ships
    // `beads_<ver>_<target>` archives containing the `bd` binary).
    let asset_prefix = tool.binary_asset_prefix();
    let archive_name = format!("{asset_prefix}_{version}_{target}");
    format!(
        "https://github.com/{}/releases/download/{}/{}.{}",
        tool.repo, tag, archive_name, ext
    )
}

/// Resolve the install destination directory for binary installs.
fn binary_install_dest(platform: &Platform) -> Result<PathBuf> {
    if platform.os == Os::Windows {
        let local_app_data = std::env::var("LOCALAPPDATA").map_err(|_| {
            DdlError::PrerequisiteMissing(
                "%LOCALAPPDATA% not set. On Windows, this should point to AppData\\Local."
                    .to_string(),
            )
        })?;
        Ok(binary_dest_dir(
            &platform.os,
            None,
            Some(PathBuf::from(local_app_data).as_path()),
        ))
    } else {
        Ok(binary_dest_dir(
            &platform.os,
            dirs::home_dir().as_deref(),
            None,
        ))
    }
}

/// Print the verification outcome banner and record it for JSON events.
fn report_verification_status(status: VerificationStatus, tool: &Tool, url: &str) {
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
            tool.name, url
        ),
        VerificationStatus::Unverified => {}
    }
    crate::output::record_install_verification(tool.name, status.label());
}

/// Print the resource budgets so the limits a download, extraction or child
/// call is held to are visible, not implicit (DDL-1z1).
fn print_budgets(verbose: bool) {
    verbose_print(
        verbose,
        &format!(
            "budgets: download ≤ {MAX_RELEASE_ASSET_BYTES} bytes; extracted binary ≤ {MAX_BINARY_BYTES} bytes; child runtime ≤ {} s; captured output ≤ {MAX_CHILD_OUTPUT_BYTES} bytes",
            child_deadline().as_secs()
        ),
    );
}

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

    let target = binary_asset_target(platform)?;
    let client = github_client()?;
    let release = fetch_latest_release(&client, tool)?;

    let tag = release["tag_name"]
        .as_str()
        .ok_or_else(|| DdlError::InstallFailed("No tag_name in release".to_string()))?;
    let version = tag.trim_start_matches('v');
    let download_url = binary_download_url(tool, tag, version, target, platform);

    verbose_print(
        verbose,
        &format!("downloading {}/releases/latest for {}", tool.repo, target),
    );

    // Determine install destination
    let dest_dir = binary_install_dest(platform)?;
    std::fs::create_dir_all(&dest_dir).map_err(DdlError::Io)?;
    let dest_path = dest_dir.join(&binary_name);

    // Verification runs BEFORE any destination mutation: the archive bytes are
    // only extracted once the checksum policy has accepted them (DDL-yts).
    print_budgets(verbose);
    let archive_bytes = fetch_release_asset(&download_url, tool.name)?;
    let status = verify_download(&download_url, &archive_bytes, tool, verbose)?;
    report_verification_status(status, tool, &download_url);

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::{Arch, Os};

    fn platform(os: Os, arch: Arch) -> Platform {
        Platform { os, arch }
    }

    #[test]
    fn test_binary_asset_target_covers_release_matrix() {
        assert_eq!(
            binary_asset_target(&platform(Os::Linux, Arch::Amd64)).unwrap(),
            "linux_amd64"
        );
        assert_eq!(
            binary_asset_target(&platform(Os::Linux, Arch::Arm64)).unwrap(),
            "linux_arm64"
        );
        assert_eq!(
            binary_asset_target(&platform(Os::Macos, Arch::Amd64)).unwrap(),
            "darwin_amd64"
        );
        assert_eq!(
            binary_asset_target(&platform(Os::Macos, Arch::Arm64)).unwrap(),
            "darwin_arm64"
        );
        assert_eq!(
            binary_asset_target(&platform(Os::Windows, Arch::Amd64)).unwrap(),
            "windows_amd64"
        );
    }

    #[test]
    fn test_binary_download_url_matches_release_asset_shape() {
        let tool = crate::platform::find_tool("wai").expect("wai is managed");
        let url = binary_download_url(
            tool,
            "v2026.11.17",
            "2026.11.17",
            "linux_amd64",
            &platform(Os::Linux, Arch::Amd64),
        );
        assert_eq!(
            url,
            "https://github.com/charly-vibes/wai/releases/download/v2026.11.17/wai_2026.11.17_linux_amd64.tar.gz"
        );
        // Windows uses .zip and an .exe binary name.
        let win = binary_download_url(
            tool,
            "v2026.11.17",
            "2026.11.17",
            "windows_amd64",
            &platform(Os::Windows, Arch::Amd64),
        );
        assert!(
            win.ends_with(".zip"),
            "windows asset must be .zip, got {win}"
        );
        // Asset-prefix override (beads ships beads_* archives for bd).
        let bd = crate::platform::find_tool("bd").expect("bd is managed");
        let bd_url = binary_download_url(
            bd,
            "v1.0.0",
            "1.0.0",
            "linux_amd64",
            &platform(Os::Linux, Arch::Amd64),
        );
        assert!(
            bd_url.contains("/beads_1.0.0_linux_amd64"),
            "bd must use the beads asset prefix, got {bd_url}"
        );
    }
}
