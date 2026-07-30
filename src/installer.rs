//! Installation chain — binary download, cargo install, brew install, scoop install.
//!
//! Fallback order: binary download → cargo install → brew/scoop install.
//! Binary download is the preferred path — no prerequisites beyond curl/wget.
//! On 404 (binary not published), ddl reports the error rather than falling
//! back to cargo (per spec — avoids unexpected behavior).

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::{DdlError, Result};
use crate::manifest::{Manifest, ToolEntry};
use crate::platform::{Os, PackageManager, Platform, Tool, MANAGED_TOOLS};

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
    Brew,
    Scoop,
    Skipped,
}

impl std::fmt::Display for InstallMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Binary => write!(f, "binary download"),
            Self::Cargo => write!(f, "cargo install"),
            Self::Brew => write!(f, "brew install"),
            Self::Scoop => write!(f, "scoop install"),
            Self::Skipped => write!(f, "skipped"),
        }
    }
}

impl InstallMethod {
    /// Check if prerequisites for this install method are met.
    pub fn check_prerequisites(&self) -> Result<()> {
        match self {
            Self::Cargo => {
                if !which("cargo").is_some() {
                    return Err(DdlError::PrerequisiteMissing(
                        "cargo is not installed. Install Rust via https://rustup.rs".to_string(),
                    ));
                }
            }
            Self::Brew => {
                if !which("brew").is_some() {
                    return Err(DdlError::PrerequisiteMissing(
                        "Homebrew is not installed. Install via https://brew.sh".to_string(),
                    ));
                }
            }
            Self::Scoop => {
                if !which("scoop").is_some() {
                    return Err(DdlError::PrerequisiteMissing(
                        "Scoop is not installed. Install via https://scoop.sh".to_string(),
                    ));
                }
            }
            Self::Binary => {
                if !which("curl").is_some() && !which("wget").is_some() {
                    return Err(DdlError::PrerequisiteMissing(
                        "curl or wget is required for binary download".to_string(),
                    ));
                }
            }
            Self::Skipped => {}
        }
        Ok(())
    }
}

/// Determine the best installation method for a tool on the current platform.
pub fn best_install_method(tool: &Tool, platform: &Platform) -> InstallMethod {
    match platform.os {
        Os::Macos => {
            if PackageManager::Brew.is_available() && !is_placeholder_formula(tool) {
                InstallMethod::Brew
            } else if PackageManager::Cargo.is_available() {
                InstallMethod::Cargo
            } else {
                InstallMethod::Binary
            }
        }
        Os::Linux => {
            if PackageManager::Cargo.is_available() {
                InstallMethod::Cargo
            } else {
                InstallMethod::Binary
            }
        }
        Os::Windows => {
            if PackageManager::Scoop.is_available() {
                InstallMethod::Scoop
            } else {
                InstallMethod::Binary
            }
        }
    }
}

/// Check if a tool is already installed on PATH.
pub fn is_tool_installed(name: &str) -> bool {
    which(name).is_some()
}

/// Get the version of an installed tool.
pub fn get_installed_version(name: &str) -> Option<String> {
    let output = Command::new(name).arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let first_line = stdout.lines().next()?;
    let parts: Vec<&str> = first_line.split_whitespace().collect();
    if parts.len() >= 2 {
        let candidate = if parts[0].to_lowercase() == name { parts[1] } else { parts[0] };
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

/// Check if a Homebrew formula is a placeholder (version 0.0.0).
pub fn is_placeholder_formula(tool: &Tool) -> bool {
    let output = Command::new("brew")
        .args(["info", "--json=v2", tool.formula_name])
        .output();
    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            stdout.contains("\"version\":\"0.0.0\"")
                || stdout.contains("\"version\": \"0.0.0\"")
        }
        _ => true,
    }
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

    let result = match method {
        InstallMethod::Binary => install_binary(tool, platform, verbose),
        InstallMethod::Cargo => install_cargo(tool, verbose),
        InstallMethod::Brew => install_brew(tool, verbose),
        InstallMethod::Scoop => install_scoop(tool, verbose),
        InstallMethod::Skipped => unreachable!(),
    };

    let detected = get_installed_version(tool.name)
        .unwrap_or_else(|| "unknown".to_string());

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

/// Install a tool via binary download from GitHub releases.
fn install_binary(tool: &Tool, platform: &Platform, _verbose: bool) -> Result<()> {
    let binary_name = if platform.os == Os::Windows {
        format!("{}.exe", tool.name)
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
        .user_agent("ddl/0.1.0")
        .build()
        .map_err(|e| DdlError::Other(format!("Failed to create HTTP client: {e}")))?;

    let releases_url = format!("https://api.github.com/repos/{}/releases/latest", tool.repo);
    let resp = client
        .get(&releases_url)
        .send()
        .map_err(|e| DdlError::Network(e))?;

    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(DdlError::InstallFailed(format!(
            "No releases found for {} — the first release may not be published yet. Try `cargo install {}` manually.",
            tool.repo, tool.crate_name
        )));
    }
    if !resp.status().is_success() {
        return Err(DdlError::InstallFailed(format!(
            "GitHub API returned {} for {}",
            resp.status(),
            releases_url
        )));
    }

    let release: serde_json::Value = resp
        .json()
        .map_err(|e| DdlError::Network(e))?;

    let tag = release["tag_name"]
        .as_str()
        .ok_or_else(|| DdlError::InstallFailed("No tag_name in release".to_string()))?;
    let version = tag.trim_start_matches('v');

    let ext = if platform.os == Os::Windows { "zip" } else { "tar.gz" };
    let archive_name = format!("{}_{}_{}", tool.name, version, target);
    let download_url = format!(
        "https://github.com/{}/releases/download/{}/{}.{}",
        tool.repo, tag, archive_name, ext
    );

    // Determine install destination
    let dest_dir = if platform.os == Os::Windows {
        let local_app_data = std::env::var("LOCALAPPDATA")
            .map_err(|_| DdlError::PrerequisiteMissing(
                "%LOCALAPPDATA% not set. On Windows, this should point to AppData\\Local.".to_string()
            ))?;
        PathBuf::from(local_app_data).join("ddl").join("bin")
    } else {
        dirs::home_dir()
            .ok_or_else(|| DdlError::Other("Cannot find home directory".to_string()))?
            .join(".ddl")
            .join("bin")
    };

    std::fs::create_dir_all(&dest_dir)
        .map_err(|e| DdlError::Io(e))?;

    let dest_path = dest_dir.join(&binary_name);

    if ext == "zip" {
        download_and_extract_zip(&download_url, &dest_path)?;
    } else {
        download_and_extract_tar_gz(&download_url, &dest_path, tool.name)?;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&dest_path, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| DdlError::Io(e))?;
    }

    ensure_on_path(&dest_dir, platform)?;

    eprintln!("  ✓ {} downloaded to {}", tool.name, dest_path.display());
    Ok(())
}

fn download_and_extract_tar_gz(url: &str, dest: &Path, binary_name: &str) -> Result<()> {
    let response = reqwest::blocking::get(url)
        .map_err(|e| DdlError::Network(e))?;

    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(DdlError::InstallFailed(format!(
            "Binary not found at {url} — the release may not include this platform yet."
        )));
    }
    if !response.status().is_success() {
        return Err(DdlError::InstallFailed(format!(
            "Download failed: HTTP {} for {}",
            response.status(), url
        )));
    }

    let bytes = response.bytes().map_err(|e| DdlError::Network(e))?;
    let decoder = flate2::read::GzDecoder::new(&bytes[..]);
    let mut archive = tar::Archive::new(decoder);

    for entry in archive.entries().map_err(|e| DdlError::Other(e.to_string()))? {
        let mut entry = entry.map_err(|e| DdlError::Other(e.to_string()))?;
        let path = entry.path().map_err(|e| DdlError::Other(e.to_string()))?;
        if path.file_name().map_or(false, |f| f == binary_name) {
            entry.unpack(dest).map_err(|e| DdlError::Io(e))?;
            return Ok(());
        }
    }

    Err(DdlError::InstallFailed(format!(
        "Binary '{}' not found in archive from {url}", binary_name
    )))
}

fn download_and_extract_zip(url: &str, dest: &Path) -> Result<()> {
    let response = reqwest::blocking::get(url)
        .map_err(|e| DdlError::Network(e))?;

    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(DdlError::InstallFailed(format!(
            "Binary not found at {url} — the release may not include Windows yet."
        )));
    }
    if !response.status().is_success() {
        return Err(DdlError::InstallFailed(format!(
            "Download failed: HTTP {} for {}",
            response.status(), url
        )));
    }

    let bytes = response.bytes().map_err(|e| DdlError::Network(e))?;
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes.to_vec()))
        .map_err(|e| DdlError::Other(e.to_string()))?;

    let binary_name = dest.file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| DdlError::Other("Invalid destination path".to_string()))?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)
            .map_err(|e| DdlError::Other(e.to_string()))?;
        let path = entry.name().to_string();

        if path.ends_with(binary_name) || path == binary_name {
            let mut out = std::fs::File::create(dest)
                .map_err(|e| DdlError::Io(e))?;
            std::io::copy(&mut entry, &mut out)
                .map_err(|e| DdlError::Io(e))?;
            return Ok(());
        }
    }

    Err(DdlError::InstallFailed(format!(
        "Binary '{binary_name}' not found in zip archive from {url}"
    )))
}

fn install_cargo(tool: &Tool, _verbose: bool) -> Result<()> {
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

fn install_brew(tool: &Tool, _verbose: bool) -> Result<()> {
    let status = Command::new("brew")
        .args(["install", tool.formula_name])
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status()
        .map_err(|e| DdlError::InstallFailed(format!("Failed to run brew: {e}")))?;

    if status.success() {
        Ok(())
    } else {
        Err(DdlError::InstallFailed(format!(
            "brew install {} exited with code {}",
            tool.formula_name,
            status.code().unwrap_or(-1)
        )))
    }
}

fn install_scoop(tool: &Tool, _verbose: bool) -> Result<()> {
    let status = Command::new("scoop")
        .args(["install", tool.formula_name])
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status()
        .map_err(|e| DdlError::InstallFailed(format!("Failed to run scoop: {e}")))?;

    if status.success() {
        Ok(())
    } else {
        Err(DdlError::InstallFailed(format!(
            "scoop install {} exited with code {}",
            tool.formula_name,
            status.code().unwrap_or(-1)
        )))
    }
}

/// Run a tool's init command after installation.
pub fn run_tool_init(tool: &Tool, _verbose: bool) -> Result<()> {
    let (cmd, args): (&str, &[&str]) = match tool.name {
        "wai" => ("wai", &["init"]),
        "dont" => ("dont", &["prime", "--plain"]),
        "ah" => ("ah", &["init"]),
        "pretender" => ("pretender", &["init"]),
        "testaruda" => ("testaruda", &["init"]),
        "fotos-mcp" => ("fotos-mcp", &["init"]),
        "fabbro" => ("fabbro", &["init"]),
        _ => return Ok(()),
    };

    // Check if tool is on PATH first
    if !is_tool_installed(cmd) {
        eprintln!("  ⚠ {} not found on PATH — skipping init", cmd);
        return Ok(());
    }

    let output = Command::new(cmd)
        .args(args)
        .output()
        .map_err(|e| {
            DdlError::InstallFailed(format!("Failed to run {} init: {e}", tool.name))
        })?;

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
            let rc_file = if dirs::home_dir().map_or(false, |h| h.join(".zshrc").exists()) {
                "~/.zshrc"
            } else if dirs::home_dir().map_or(false, |h| h.join(".bashrc").exists()) {
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

fn which(cmd: &str) -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|paths| {
        for dir in std::env::split_paths(&paths) {
            let full = dir.join(cmd);
            if full.is_file() {
                return Some(full);
            }
            #[cfg(windows)]
            {
                let full_exe = dir.join(format!("{cmd}.exe"));
                if full_exe.is_file() {
                    return Some(full_exe);
                }
            }
        }
        None
    })
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