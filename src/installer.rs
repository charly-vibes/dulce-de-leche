//! Installation chain — binary download (primary), cargo install (fallback), npm install.
//!
//! Policy (DDL-ei3): prebuilt binary first on every platform — no prerequisites
//! beyond curl/wget. When the release has no binary for the platform (404),
//! fall back to `cargo install` if cargo is available; otherwise fail with an
//! error naming both remedies. npm-distributed tools (incitaciones) always
//! install via npm. Homebrew and Scoop are not part of ddl's install decisions;
//! their formulas/manifests remain published for manual installs.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Print a trace message to stderr when `verbose` is enabled.
/// Used to show what the installer is doing without cluttering normal output.
fn verbose_print(verbose: bool, msg: &str) {
    if verbose {
        eprintln!("  · {msg}");
    }
}

use crate::error::{DdlError, Result};
use crate::manifest::{Manifest, ToolEntry};
use crate::platform::{MANAGED_TOOLS, Os, PackageManager, Platform, Tool};

/// Result of a single tool installation attempt.
#[derive(Debug, Clone)]
pub struct InstallResult {
    pub tool: &'static str,
    pub success: bool,
    pub method: InstallMethod,
    pub version: String,
    pub message: String,
}

/// The method used to install a tool.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallMethod {
    Binary,
    Cargo,
    Npm,
    Skipped,
}

impl std::fmt::Display for InstallMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Binary => write!(f, "binary download"),
            Self::Cargo => write!(f, "cargo install"),
            Self::Npm => write!(f, "npm install"),
            Self::Skipped => write!(f, "skipped"),
        }
    }
}

impl InstallMethod {
    /// Check if prerequisites for this install method are met.
    pub fn check_prerequisites(&self) -> Result<()> {
        match self {
            Self::Cargo => {
                if which("cargo").is_none() {
                    return Err(DdlError::PrerequisiteMissing(
                        "cargo is not installed. Install Rust via https://rustup.rs".to_string(),
                    ));
                }
            }
            Self::Binary => {
                if which("curl").is_none() && which("wget").is_none() {
                    return Err(DdlError::PrerequisiteMissing(
                        "curl or wget is required for binary download".to_string(),
                    ));
                }
            }
            Self::Npm => {
                // `npm install -g` is the only install path for npm tools —
                // npx alone can't fulfill it. npm ships with npx, so requiring
                // npm here matches the implementation exactly.
                if which("npm").is_none() {
                    return Err(DdlError::PrerequisiteMissing(
                        "npm is not installed. Install Node.js via https://nodejs.org".to_string(),
                    ));
                }
            }
            Self::Skipped => {}
        }
        Ok(())
    }
}

/// Determine the installation method for a tool on the current platform.
///
/// Policy (DDL-ei3): prebuilt binary first on every platform; cargo is a
/// runtime fallback applied when the binary download fails because no release
/// binary was published (see `fallback_after_binary_failure`). npm-distributed
/// tools (incitaciones) always install via npm.
pub fn best_install_method(tool: &Tool, _platform: &Platform) -> InstallMethod {
    if tool.npm_package.is_some() {
        InstallMethod::Npm
    } else {
        InstallMethod::Binary
    }
}

/// Decide the follow-up method when the binary download path fails because no
/// release binary was published for this platform.
///
/// Cargo is the only fallback (DDL-ei3). Returns `None` when cargo is not
/// available — the caller then fails with an error naming both remedies.
pub fn fallback_after_binary_failure(cargo_available: bool) -> Option<InstallMethod> {
    cargo_available.then_some(InstallMethod::Cargo)
}

/// Check if a tool is already installed on PATH.
pub fn is_tool_installed(name: &str) -> bool {
    which(name).is_some()
}

/// Get the version of an installed tool.
pub fn get_installed_version(name: &str) -> Option<String> {
    // npm packages don't support `--version` uniformly — ask npm instead.
    if crate::platform::find_tool(name).is_some_and(|t| t.npm_package.is_some()) {
        return get_npm_global_version(name);
    }
    let output = Command::new(name).arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let first_line = stdout.lines().next()?;
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
            candidate.trim_start_matches('v')
        };
        Some(version.to_string())
    } else {
        Some(first_line.to_string())
    }
}

/// Get the globally installed npm version of a package via `npm list -g`.
fn get_npm_global_version(package: &str) -> Option<String> {
    let output = npm_command("npm")
        .args(["list", "-g", package, "--depth=0"])
        .output()
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

/// Install a tool using the best available method.
pub fn install_tool(
    tool: &Tool,
    platform: &Platform,
    manifest: &mut Manifest,
    verbose: bool,
) -> InstallResult {
    let method = best_install_method(tool, platform);

    if is_tool_installed(tool.name) {
        let ver = get_installed_version(tool.name).unwrap_or_else(|| "?".to_string());
        return InstallResult {
            tool: tool.name,
            success: true,
            method: InstallMethod::Skipped,
            version: ver.clone(),
            message: format!("{} v{} is already installed", tool.name, ver),
        };
    }

    let (method, result) = match method {
        InstallMethod::Binary => match install_binary(tool, platform, verbose) {
            Ok(()) => (InstallMethod::Binary, Ok(())),
            // Definitive miss (no release asset for this platform): fall back
            // to cargo when available. Transient failures do NOT fall back.
            Err(DdlError::NoReleaseBinary { .. }) => {
                match fallback_after_binary_failure(PackageManager::Cargo.is_available()) {
                    Some(InstallMethod::Cargo) => {
                        verbose_print(
                            verbose,
                            &format!(
                                "⚠ {} binary not yet available for this platform — using cargo install instead",
                                tool.name
                            ),
                        );
                        (InstallMethod::Cargo, install_cargo(tool, verbose))
                    }
                    _ => (
                        InstallMethod::Binary,
                        Err(DdlError::InstallFailed(format!(
                            "⚠ {} binary not yet available for this platform, and cargo is not installed. \
                             Install Rust (https://rustup.rs) or download the binary manually from \
                             https://github.com/{}/releases",
                            tool.name, tool.repo
                        ))),
                    ),
                }
            }
            Err(e) => (InstallMethod::Binary, Err(e)),
        },
        other => {
            let result = execute_install(&other, tool, platform, verbose);
            (other, result)
        }
    };

    let detected = get_installed_version(tool.name).unwrap_or_else(|| "unknown".to_string());

    match result {
        Ok(()) => {
            manifest.set_tool(
                tool.name,
                ToolEntry {
                    installed: detected.clone(),
                    source: method.to_string(),
                    status: "installed".to_string(),
                    compatible: ">=0.0.0".to_string(),
                },
            );
            InstallResult {
                tool: tool.name,
                success: true,
                method: method.clone(),
                version: detected.clone(),
                message: format!("{} v{} installed via {}", tool.name, detected, method),
            }
        }
        Err(e) => {
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
                method,
                version: String::new(),
                message: format!("{} failed: {}", tool.name, e),
            }
        }
    }
}

/// Dispatch a single install attempt for the given method.
fn execute_install(
    method: &InstallMethod,
    tool: &Tool,
    platform: &Platform,
    verbose: bool,
) -> Result<()> {
    match method {
        InstallMethod::Binary => install_binary(tool, platform, verbose),
        InstallMethod::Cargo => install_cargo(tool, verbose),
        InstallMethod::Npm => install_npm(tool, verbose),
        InstallMethod::Skipped => unreachable!(),
    }
}

/// Dispatch a single upgrade attempt for the given method.
fn execute_upgrade(
    method: &InstallMethod,
    tool: &Tool,
    platform: &Platform,
    verbose: bool,
) -> Result<()> {
    match method {
        InstallMethod::Binary => upgrade_binary(tool, platform, verbose),
        InstallMethod::Cargo => upgrade_cargo(tool, verbose),
        InstallMethod::Npm => upgrade_npm(tool, verbose),
        InstallMethod::Skipped => unreachable!(),
    }
}

/// Install a tool via binary download from GitHub releases.
fn install_binary(tool: &Tool, platform: &Platform, verbose: bool) -> Result<()> {
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

    let client = reqwest::blocking::Client::builder()
        .user_agent(format!("ddl/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| DdlError::Other(format!("Failed to create HTTP client: {e}")))?;

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
    let archive_name = format!("{}_{}_{}", tool.name, version, target);
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
        PathBuf::from(local_app_data).join("ddl").join("bin")
    } else {
        dirs::home_dir()
            .ok_or_else(|| DdlError::Other("Cannot find home directory".to_string()))?
            .join(".ddl")
            .join("bin")
    };

    std::fs::create_dir_all(&dest_dir).map_err(DdlError::Io)?;

    let dest_path = dest_dir.join(&binary_name);

    if ext == "zip" {
        download_and_extract_zip(&download_url, &dest_path, tool.name)?;
    } else {
        download_and_extract_tar_gz(&download_url, &dest_path, tool.name)?;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&dest_path, std::fs::Permissions::from_mode(0o755))
            .map_err(DdlError::Io)?;
    }

    ensure_on_path(&dest_dir, platform)?;

    eprintln!("  ✓ {} downloaded to {}", tool.name, dest_path.display());
    Ok(())
}

fn download_and_extract_tar_gz(url: &str, dest: &Path, tool_name: &str) -> Result<()> {
    let response = reqwest::blocking::get(url).map_err(DdlError::Network)?;

    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(DdlError::NoReleaseBinary {
            tool: tool_name.to_string(),
            url: url.to_string(),
        });
    }
    if !response.status().is_success() {
        return Err(DdlError::InstallFailed(format!(
            "Download failed: HTTP {} for {}",
            response.status(),
            url
        )));
    }

    let bytes = response.bytes().map_err(DdlError::Network)?;
    let decoder = flate2::read::GzDecoder::new(&bytes[..]);
    let mut archive = tar::Archive::new(decoder);

    for entry in archive
        .entries()
        .map_err(|e| DdlError::Other(e.to_string()))?
    {
        let mut entry = entry.map_err(|e| DdlError::Other(e.to_string()))?;
        let path = entry.path().map_err(|e| DdlError::Other(e.to_string()))?;
        if path.file_name().is_some_and(|f| f == tool_name) {
            entry.unpack(dest).map_err(DdlError::Io)?;
            return Ok(());
        }
    }

    Err(DdlError::InstallFailed(format!(
        "Binary '{}' not found in archive from {url}",
        tool_name
    )))
}

fn download_and_extract_zip(url: &str, dest: &Path, tool_name: &str) -> Result<()> {
    let response = reqwest::blocking::get(url).map_err(DdlError::Network)?;

    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(DdlError::NoReleaseBinary {
            tool: tool_name.to_string(),
            url: url.to_string(),
        });
    }
    if !response.status().is_success() {
        return Err(DdlError::InstallFailed(format!(
            "Download failed: HTTP {} for {}",
            response.status(),
            url
        )));
    }

    let bytes = response.bytes().map_err(DdlError::Network)?;
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes.to_vec()))
        .map_err(|e| DdlError::Other(e.to_string()))?;

    let binary_name = dest
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| DdlError::Other("Invalid destination path".to_string()))?;

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| DdlError::Other(e.to_string()))?;
        let path = entry.name().to_string();

        if path.ends_with(binary_name) || path == binary_name {
            let mut out = std::fs::File::create(dest).map_err(DdlError::Io)?;
            std::io::copy(&mut entry, &mut out).map_err(DdlError::Io)?;
            return Ok(());
        }
    }

    Err(DdlError::InstallFailed(format!(
        "Binary '{binary_name}' not found in zip archive from {url}"
    )))
}

fn install_cargo(tool: &Tool, verbose: bool) -> Result<()> {
    verbose_print(
        verbose,
        &format!("running: cargo install {}", tool.crate_name),
    );
    let status = Command::new("cargo")
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
fn install_npm(tool: &Tool, verbose: bool) -> Result<()> {
    verbose_print(
        verbose,
        &format!("running: npm install -g {}", tool.crate_name),
    );
    let status = npm_command("npm")
        .args(["install", "-g", tool.crate_name])
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status()
        .map_err(|e| DdlError::InstallFailed(format!("Failed to run npm: {e}")))?;

    if status.success() {
        Ok(())
    } else {
        Err(DdlError::InstallFailed(format!(
            "npm install -g {} exited with code {}",
            tool.crate_name,
            status.code().unwrap_or(-1)
        )))
    }
}

/// Run a tool's init command after installation.
pub fn run_tool_init(tool: &Tool, verbose: bool) -> Result<()> {
    let (cmd, args): (&str, &[&str]) = match tool.name {
        "wai" => ("wai", &["init"]),
        "dont" => ("dont", &["prime", "--plain"]),
        "ah" => ("ah", &["init"]),
        "pretender" => ("pretender", &["init"]),
        "testaruda" => ("testaruda", &["init"]),
        "vampiro" => ("vampiro", &["init"]),
        "fotos-mcp" => ("fotos-mcp", &["init"]),
        "fabbro" => ("fabbro", &["init"]),
        _ => return Ok(()),
    };

    // Check if tool is on PATH first
    if !is_tool_installed(cmd) {
        eprintln!("  ⚠ {} not found on PATH — skipping init", cmd);
        return Ok(());
    }

    verbose_print(verbose, &format!("running: {} init", cmd));

    let output = Command::new(cmd)
        .args(args)
        .output()
        .map_err(|e| DdlError::InstallFailed(format!("Failed to run {} init: {e}", tool.name)))?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let first_line = stdout.lines().next().unwrap_or("");
        if !first_line.is_empty() {
            eprintln!("  ✓ {} initialized: {}", tool.name, first_line);
        } else {
            eprintln!("  ✓ {} initialized", tool.name);
        }
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!(
            "  ⚠ {} init exited with code {}: {}",
            tool.name,
            output.status.code().unwrap_or(-1),
            stderr.lines().next().unwrap_or("unknown error")
        );
        // Don't fail the whole install — init is advisory
        Ok(())
    }
}

fn ensure_on_path(dir: &Path, platform: &Platform) -> Result<()> {
    let dir_str = dir.to_string_lossy().to_string();

    if let Some(paths) = std::env::var_os("PATH") {
        for path in std::env::split_paths(&paths) {
            if path == dir {
                return Ok(());
            }
        }
    }

    match platform.os {
        Os::Windows => {
            eprintln!("  ⚠ Add {} to your PATH environment variable", dir_str);
            eprintln!("     set PATH=%PATH%;{}", dir_str);
        }
        _ => {
            let rc_file = if dirs::home_dir().is_some_and(|h| h.join(".zshrc").exists()) {
                "~/.zshrc"
            } else if dirs::home_dir().is_some_and(|h| h.join(".bashrc").exists()) {
                "~/.bashrc"
            } else {
                "your shell config"
            };
            eprintln!("  ⚠ Add {} to your PATH", dir_str);
            eprintln!("     export PATH=\"{}:$PATH\"", dir_str);
            eprintln!("     Add the above to {rc_file}");
        }
    }
    Ok(())
}

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
            Ok(()) => (InstallMethod::Binary, Ok(())),
            Err(DdlError::NoReleaseBinary { .. }) => {
                match fallback_after_binary_failure(PackageManager::Cargo.is_available()) {
                    Some(InstallMethod::Cargo) => {
                        verbose_print(
                            verbose,
                            &format!(
                                "⚠ {} binary not yet available for this platform — using cargo install instead",
                                tool.name
                            ),
                        );
                        (InstallMethod::Cargo, upgrade_cargo(tool, verbose))
                    }
                    _ => (
                        InstallMethod::Binary,
                        Err(DdlError::InstallFailed(format!(
                            "⚠ {} binary not yet available for this platform, and cargo is not installed. \
                             Install Rust (https://rustup.rs) or download the binary manually from \
                             https://github.com/{}/releases",
                            tool.name, tool.repo
                        ))),
                    ),
                }
            }
            Err(e) => (InstallMethod::Binary, Err(e)),
        },
        other => {
            let result = execute_upgrade(&other, tool, platform, verbose);
            (other, result)
        }
    };

    let new_version = get_installed_version(tool.name);

    match result {
        Ok(()) => {
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
                    message: format!("{} upgraded to v{} via {}", tool.name, ver, method),
                }
            } else {
                InstallResult {
                    tool: tool.name,
                    success: true,
                    method: InstallMethod::Skipped,
                    version: ver.clone(),
                    message: format!("{} v{} is already up to date", tool.name, ver),
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
fn upgrade_cargo(tool: &Tool, verbose: bool) -> Result<()> {
    verbose_print(
        verbose,
        &format!("running: cargo install {}", tool.crate_name),
    );
    // `cargo install` is idempotent — it upgrades if already installed
    let status = Command::new("cargo")
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
fn upgrade_npm(tool: &Tool, verbose: bool) -> Result<()> {
    verbose_print(
        verbose,
        &format!("running: npm install -g {}@latest", tool.crate_name),
    );
    let status = npm_command("npm")
        .args(["install", "-g", &format!("{}@latest", tool.crate_name)])
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status()
        .map_err(|e| DdlError::InstallFailed(format!("Failed to run npm: {e}")))?;

    if status.success() {
        Ok(())
    } else {
        Err(DdlError::InstallFailed(format!(
            "npm install -g {}@latest exited with code {}",
            tool.crate_name,
            status.code().unwrap_or(-1)
        )))
    }
}

/// Upgrade a tool installed via binary download (re-download).
fn upgrade_binary(tool: &Tool, platform: &Platform, verbose: bool) -> Result<()> {
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

fn which(cmd: &str) -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|paths| {
        for dir in std::env::split_paths(&paths) {
            let full = dir.join(cmd);
            if full.is_file() {
                return Some(full);
            }
            #[cfg(windows)]
            {
                let full_exe = dir.join(std::path::PathBuf::from(cmd).with_extension("exe"));
                if full_exe.is_file() {
                    return Some(full_exe);
                }
                // npm-ecosystem tools are .cmd shims on Windows — probe them
                // so check_prerequisites doesn't fail on a working npm.
                for ext in ["cmd", "bat"] {
                    let shim = dir.join(format!("{cmd}.{ext}"));
                    if shim.is_file() {
                        return Some(shim);
                    }
                }
            }
        }
        None
    })
}

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

    let client = reqwest::blocking::Client::builder()
        .user_agent(format!("ddl/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .ok()?;

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

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify verbose_print exists and accepts both values without panicking.
    /// The actual stderr output is verified by integration tests that run
    /// the ddl binary with --verbose.
    #[test]
    fn test_verbose_print_accepts_bool() {
        // Must not panic for either value
        verbose_print(true, "trace message");
        verbose_print(false, "trace message");
    }
}
