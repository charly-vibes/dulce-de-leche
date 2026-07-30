//! Installation chain — binary download, cargo install, brew install, scoop install.
//!
//! Fallback order: binary download → cargo install → brew/scoop install.
//! Binary download is the preferred path — no prerequisites beyond curl/wget.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::{DdlError, Result};
use crate::manifest::Manifest;
use crate::platform::{Os, PackageManager, Platform, Tool, MANAGED_TOOLS};

/// Result of a single tool installation attempt.
#[derive(Debug, Clone)]
pub struct InstallResult {
    pub tool: &'static str,
    pub success: bool,
    pub method: InstallMethod,
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
            // Prefer brew if available and not a placeholder
            if PackageManager::Brew.is_available() && !is_placeholder_formula(tool) {
                InstallMethod::Brew
            } else if PackageManager::Cargo.is_available() {
                InstallMethod::Cargo
            } else {
                InstallMethod::Binary
            }
        }
        Os::Linux => {
            // Prefer binary download, fallback to cargo
            if PackageManager::Cargo.is_available() {
                InstallMethod::Cargo
            } else {
                InstallMethod::Binary
            }
        }
        Os::Windows => {
            // Prefer scoop if available, fallback to binary
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

/// Check if a Homebrew formula is a placeholder (version 0.0.0).
pub fn is_placeholder_formula(tool: &Tool) -> bool {
    // Query the brew formula info and check if version is 0.0.0
    let output = Command::new("brew")
        .args(["info", "--json=v2", tool.formula_name])
        .output();
    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            // Check if the formula has version 0.0.0
            stdout.contains("\"version\":\"0.0.0\"")
                || stdout.contains("\"version\": \"0.0.0\"")
        }
        _ => {
            // If brew info fails (formula doesn't exist yet), treat as placeholder
            true
        }
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

    // Check if already installed
    if is_tool_installed(tool.name) {
        return InstallResult {
            tool: tool.name,
            success: true,
            method: InstallMethod::Skipped,
            message: format!("{} is already installed", tool.name),
        };
    }

    let result = match method {
        InstallMethod::Binary => install_binary(tool, platform, verbose),
        InstallMethod::Cargo => install_cargo(tool, verbose),
        InstallMethod::Brew => install_brew(tool, verbose),
        InstallMethod::Scoop => install_scoop(tool, verbose),
        InstallMethod::Skipped => unreachable!(),
    };

    match result {
        Ok(()) => {
            manifest.set_tool(
                tool.name,
                crate::manifest::ToolEntry {
                    installed: "unknown".to_string(),
                    source: method.to_string(),
                    status: "installed".to_string(),
                    compatible: ">=0.0.0".to_string(),
                },
            );
            InstallResult {
                tool: tool.name,
                success: true,
                method: method.clone(),
                message: format!("{} installed via {}", tool.name, method.clone()),
            }
        }
        Err(e) => {
            manifest.set_tool(
                tool.name,
                crate::manifest::ToolEntry {
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

    // Fetch latest release tag from GitHub API
    let client = reqwest::blocking::Client::builder()
        .user_agent("ddl/0.1.0")
        .build()
        .map_err(|e| DdlError::Other(format!("Failed to create HTTP client: {e}")))?;

    let releases_url = format!("https://api.github.com/repos/{}/releases/latest", tool.repo);
    let resp = client
        .get(&releases_url)
        .send()
        .map_err(|e| DdlError::Network(e))?;

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

    // Build download URL
    let ext = if platform.os == Os::Windows { "zip" } else { "tar.gz" };
    let archive_name = format!("{}_{}_{}", tool.name, version, target);
    let download_url = format!(
        "https://github.com/{}/releases/download/{}/{}.{}",
        tool.repo, tag, archive_name, ext
    );

    // Determine install destination
    let dest_dir = if platform.os == Os::Windows {
        let local_app_data = std::env::var("LOCALAPPDATA")
            .map_err(|_| DdlError::Other("LOCALAPPDATA not set".to_string()))?;
        PathBuf::from(local_app_data).join("ddl").join("bin")
    } else {
        // Try to install to a common location, fallback to ~/.ddl/bin
        let home = dirs::home_dir()
            .ok_or_else(|| DdlError::Other("Cannot find home directory".to_string()))?;
        home.join(".ddl").join("bin")
    };

    std::fs::create_dir_all(&dest_dir)
        .map_err(|e| DdlError::Io(e))?;

    let dest_path = dest_dir.join(&binary_name);

    if ext == "zip" {
        download_and_extract_zip(&download_url, &dest_path)?;
    } else {
        download_and_extract_tar_gz(&download_url, &dest_path, tool.name)?;
    }

    // Make executable on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&dest_path, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| DdlError::Io(e))?;
    }

    // Add to PATH if needed
    ensure_on_path(&dest_dir, platform)?;

    eprintln!("  ✓ {} downloaded to {}", tool.name, dest_path.display());
    Ok(())
}

/// Download a `.tar.gz` archive, extract the binary, and place it at `dest`.
fn download_and_extract_tar_gz(url: &str, dest: &Path, binary_name: &str) -> Result<()> {
    let response = reqwest::blocking::get(url)
        .map_err(|e| DdlError::Network(e))?;

    if !response.status().is_success() {
        return Err(DdlError::InstallFailed(format!(
            "Download failed: HTTP {} for {}",
            response.status(),
            url
        )));
    }

    let bytes = response
        .bytes()
        .map_err(|e| DdlError::Network(e))?;

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
        "Binary '{}' not found in archive",
        binary_name
    )))
}

/// Download a `.zip` archive, extract the binary, and place it at `dest`.
fn download_and_extract_zip(url: &str, dest: &Path) -> Result<()> {
    let response = reqwest::blocking::get(url)
        .map_err(|e| DdlError::Network(e))?;

    if !response.status().is_success() {
        return Err(DdlError::InstallFailed(format!(
            "Download failed: HTTP {} for {}",
            response.status(),
            url
        )));
    }

    let bytes = response
        .bytes()
        .map_err(|e| DdlError::Network(e))?;

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
        let path = entry
            .name()
            .to_string();

        if path.ends_with(binary_name) || path == binary_name {
            let mut out = std::fs::File::create(dest)
                .map_err(|e| DdlError::Io(e))?;
            std::io::copy(&mut entry, &mut out)
                .map_err(|e| DdlError::Io(e))?;
            return Ok(());
        }
    }

    Err(DdlError::InstallFailed(format!(
        "Binary '{}' not found in zip archive",
        binary_name
    )))
}

/// Install a tool via `cargo install`.
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

/// Install a tool via `brew install`.
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

/// Install a tool via `scoop install`.
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
        _ => return Ok(()), // some tools don't have init commands
    };

    let status = Command::new(cmd)
        .args(args)
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status()
        .map_err(|e| {
            DdlError::InstallFailed(format!("Failed to run {} init: {e}", tool.name))
        })?;

    if status.success() {
        eprintln!("  ✓ {} initialized", tool.name);
        Ok(())
    } else {
        eprintln!("  ⚠ {} init exited with code {}", tool.name, status.code().unwrap_or(-1));
        // Don't fail the whole install — init is advisory
        Ok(())
    }
}

/// Ensure a directory is on PATH (or suggest adding it).
fn ensure_on_path(dir: &Path, platform: &Platform) -> Result<()> {
    let dir_str = dir.to_string_lossy().to_string();

    // Check if already on PATH
    if let Some(paths) = std::env::var_os("PATH") {
        for path in std::env::split_paths(&paths) {
            if path == dir {
                return Ok(()); // already on PATH
            }
        }
    }

    match platform.os {
        Os::Windows => {
            eprintln!("  ⚠ Add {} to your PATH", dir_str);
        }
        _ => {
            eprintln!("  ⚠ Add {} to your PATH", dir_str);
            eprintln!("     export PATH=\"{}:$PATH\"", dir_str);
            eprintln!("     Or add the above to your shell config (~/.zshrc, ~/.bashrc)");
        }
    }
    Ok(())
}

/// Check if a command exists on PATH.
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

/// Get the version of a tool from its `--version` output.
pub fn get_tool_version(name: &str) -> Option<String> {
    let output = Command::new(name).arg("--version").output().ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Parse first line: "wai 2026.5.1" or "ddl 0.1.0"
    stdout
        .lines()
        .next()?
        .split_whitespace()
        .nth(1)
        .map(|s| s.to_string())
}

/// Check prerequisites for a given install method.
pub fn check_prerequisites(method: &InstallMethod) -> Result<()> {
    match method {
        InstallMethod::Cargo => {
            if !which("cargo").is_some() {
                return Err(DdlError::PrerequisiteMissing(
                    "cargo is not installed. Install Rust via https://rustup.rs".to_string(),
                ));
            }
        }
        InstallMethod::Brew => {
            if !which("brew").is_some() {
                return Err(DdlError::PrerequisiteMissing(
                    "Homebrew is not installed. Install via https://brew.sh".to_string(),
                ));
            }
        }
        InstallMethod::Scoop => {
            if !which("scoop").is_some() {
                return Err(DdlError::PrerequisiteMissing(
                    "Scoop is not installed. Install via https://scoop.sh".to_string(),
                ));
            }
        }
        InstallMethod::Binary => {
            // Binary download needs curl or wget, but these are pre-installed on most systems
            if !which("curl").is_some() && !which("wget").is_some() {
                return Err(DdlError::PrerequisiteMissing(
                    "curl or wget is required for binary download".to_string(),
                ));
            }
        }
        InstallMethod::Skipped => {}
    }
    Ok(())
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