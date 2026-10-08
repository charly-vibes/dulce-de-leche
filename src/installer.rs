//! Installation chain — binary download (primary, sha256-verified), cargo
//! install (fallback), npm install.
//!
//! Policy (DDL-ei3): prebuilt binary first on every platform — no prerequisites
//! beyond curl/wget. Binary downloads are verified against the release's
//! published checksums (DDL-5ph) before extraction; mismatch aborts the
//! install. When the release has no binary for the platform (404),
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
    // Use the npm package name, which may be scoped and differ from the
    // binary name (e.g. binary `openspec`, package `@fission-ai/openspec`).
    if let Some(tool) = crate::platform::find_tool(name)
        && tool.npm_package.is_some()
    {
        return get_npm_global_version(tool.npm_package.unwrap_or(name));
    }
    let output = Command::new(name).arg("--version").output().ok()?;
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
fn parse_version_line(name: &str, first_line: &str) -> Option<String> {
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
    let output = Command::new(path).arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let first_line = stdout.lines().next()?;
    parse_version_line(name, first_line)
}

/// Destination directory for binary installs. Pure so tests can inject the
/// home / LOCALAPPDATA roots.
fn binary_dest_dir(os: &Os, home: Option<&Path>, local_app_data: Option<&Path>) -> PathBuf {
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
fn binary_dest_path(tool: &Tool, platform: &Platform) -> PathBuf {
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
fn resolve_with_dest(binary_name: &str, dest_dir: Option<&Path>) -> Option<String> {
    get_installed_version(binary_name)
        .or_else(|| dest_dir.and_then(|d| probe_version_at(&d.join(binary_name), binary_name)))
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

/// GitHub access token for API requests, if available.
///
/// Reads `GITHUB_TOKEN` then `GH_TOKEN` (both conventions are common).
/// Unauthenticated api.github.com requests are limited to 60/hour per IP;
/// shared CI runners routinely exhaust that quota, so ddl attaches the
/// token whenever the environment provides one (Actions runners always do).
fn github_token() -> Option<String> {
    ["GITHUB_TOKEN", "GH_TOKEN"]
        .iter()
        .find_map(|var| std::env::var(var).ok().filter(|t| !t.is_empty()))
}

/// Build an HTTP client for GitHub API calls: ddl user-agent plus a Bearer
/// token when one is available (see `github_token`).
fn github_client() -> Result<reqwest::blocking::Client> {
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
            // to cargo when available — but only for tools with a real
            // crates.io package (Go/npm tools have none, and installing the
            // wrong crate would be actively harmful).
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
                        (InstallMethod::Cargo, install_cargo(tool, verbose))
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
            let result = execute_install(&other, tool, platform, verbose);
            (other, result)
        }
    };

    let detected = resolve_with_dest(tool.name, Some(&binary_dest_path(tool, platform)))
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

    let archive_bytes = fetch_release_asset(&download_url, tool.name)?;
    verify_download(&download_url, &archive_bytes, tool, verbose)?;

    if ext == "zip" {
        extract_zip(&archive_bytes, &dest_path, &download_url)?;
    } else {
        extract_tar_gz(&archive_bytes, &dest_path, tool.name, &download_url)?;
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

/// SHA-256 of `bytes` as lowercase hex.
fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        hex.push_str(&format!("{byte:02x}"));
    }
    hex
}

/// Parse a shasum-style checksum manifest (`<sha256>  <filename>` per line;
/// two spaces per `shasum -a 256`, single space and bsd-style `"<hex>" *name`
/// also accepted). Lines without a 64-char hex digest are skipped.
/// Returns `None` when nothing valid remains.
fn parse_checksums(content: &str) -> Option<std::collections::HashMap<String, String>> {
    let mut map = std::collections::HashMap::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        // bsd-style: "<hex>" *filename
        let (digest, name) = if let Some(quoted) = line.strip_prefix('"') {
            let Some((digest, rest)) = quoted.split_once('"') else {
                continue;
            };
            let rest = rest.trim_start();
            let name = rest.trim_start_matches('*');
            (digest, name)
        } else {
            let Some((digest, rest)) = line.split_once(char::is_whitespace) else {
                continue;
            };
            // sha256sum binary mode prefixes the filename with `*`
            (digest, rest.trim_start().trim_start_matches('*'))
        };
        let digest = digest.to_lowercase();
        if digest.len() != 64 || !digest.chars().all(|c| c.is_ascii_hexdigit()) {
            continue;
        }
        if name.is_empty() {
            continue;
        }
        map.insert(name.to_string(), digest);
    }
    (!map.is_empty()).then_some(map)
}

/// Verify `bytes` hash to `expected` (case-insensitive). Mismatch is a hard
/// `ChecksumMismatch` naming the tool — the caller must not install the asset.
fn verify_checksum(bytes: &[u8], expected: &str, tool: &str, url: &str) -> Result<()> {
    let actual = sha256_hex(bytes);
    if actual == expected.to_lowercase() {
        Ok(())
    } else {
        Err(DdlError::ChecksumMismatch {
            tool: tool.to_string(),
            expected: expected.to_lowercase(),
            actual,
            url: url.to_string(),
        })
    }
}

/// Look up the published sha256 for the asset at `archive_url`, printing an
/// accurate skip-reason (verbose) whenever `None` is returned.
///
/// Conventions checked in order:
/// 1. `checksums.txt` next to the asset (shasum format — ddl's own release
///    workflow publishes this), with the archive looked up by filename
/// 2. a per-asset `<archive>.sha256` file containing a bare digest
///
/// Returns `None` when the release publishes no usable checksum source or the
/// asset is absent from the manifest — verification is then skipped rather
/// than failing, since aborting would break installs for family tools that
/// don't publish checksums yet. When a checksum IS published, mismatch is
/// fatal (see `verify_checksum`).
fn fetch_expected_checksum(archive_url: &str, verbose: bool) -> Option<String> {
    let (base, archive_name) = archive_url.rsplit_once('/')?;

    let checksums_url = format!("{base}/checksums.txt");
    if let Ok(resp) = reqwest::blocking::get(&checksums_url)
        && resp.status().is_success()
        && let Ok(body) = resp.text()
    {
        if let Some(expected) = parse_checksums(&body).and_then(|m| m.get(archive_name).cloned()) {
            return Some(expected);
        }
        verbose_print(
            verbose,
            &format!(
                "⚠ {archive_name} is not listed in this release's checksums.txt — skipping verification"
            ),
        );
        return None;
    }

    let per_asset_url = format!("{archive_url}.sha256");
    if let Ok(resp) = reqwest::blocking::get(&per_asset_url)
        && resp.status().is_success()
        && let Ok(body) = resp.text()
    {
        let digest = body.split_whitespace().next().unwrap_or("").to_lowercase();
        if digest.len() == 64 && digest.chars().all(|c| c.is_ascii_hexdigit()) {
            return Some(digest);
        }
    }

    verbose_print(
        verbose,
        &format!("⚠ release publishes no checksum for {archive_name} — skipping verification"),
    );
    None
}

/// Verify a downloaded release archive against the checksum published for the
/// same release. Skip-warning is owned by `fetch_expected_checksum`.
fn verify_download(
    archive_url: &str,
    archive_bytes: &[u8],
    tool: &Tool,
    verbose: bool,
) -> Result<()> {
    match fetch_expected_checksum(archive_url, verbose) {
        Some(expected) => {
            verify_checksum(archive_bytes, &expected, tool.name, archive_url)?;
            verbose_print(
                verbose,
                &format!("✓ {} sha256 verified against release checksum", tool.name),
            );
            Ok(())
        }
        None => Ok(()),
    }
}

/// Fetch a release archive, mapping the release-404
/// case to `NoReleaseBinary` and any other non-success to `InstallFailed`.
fn fetch_release_asset(url: &str, tool_name: &str) -> Result<bytes::Bytes> {
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
    response.bytes().map_err(DdlError::Network)
}

fn extract_tar_gz(bytes: &[u8], dest: &Path, tool_name: &str, url: &str) -> Result<()> {
    let decoder = flate2::read::GzDecoder::new(bytes);
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

fn extract_zip(bytes: &[u8], dest: &Path, url: &str) -> Result<()> {
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
    // The npm package may be scoped (e.g. @fission-ai/openspec) — always use
    // npm_package, falling back to crate_name for registry-style entries.
    let package = tool.npm_package.unwrap_or(tool.crate_name);
    verbose_print(verbose, &format!("running: npm install -g {package}"));
    let status = npm_command("npm")
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

/// A tool-init step that failed during the cascade.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitFailure {
    pub tool: String,
    pub detail: String,
}

/// The init command for a tool, if it has one.
///
/// `non_interactive` selects prompt-free variants: wizards (pretender) would
/// otherwise leak interactive prompt text into `--yes` output, and config
/// prompts (openspec, bd) would hang CI.
///
/// Tools without an init command (fotos-mcp, incitaciones, turu) return None
/// — ddl must skip them silently, never invoke a bogus `init` subcommand.
pub fn init_command(
    tool_name: &str,
    non_interactive: bool,
) -> Option<(&'static str, Vec<&'static str>)> {
    let (cmd, args): (&'static str, &[&'static str]) = match tool_name {
        "wai" => ("wai", &["init"]),
        // `dont init` first: `prime` fails when no .dont/ project exists yet.
        "dont" => ("dont", &["init"]),
        "ah" => ("ah", &["init"]),
        "pretender" if non_interactive => ("pretender", &["init", "--non-interactive"]),
        "pretender" => ("pretender", &["init"]),
        "testaruda" => ("testaruda", &["init"]),
        "vampiro" => ("vampiro", &["init"]),
        // bd init is interactive by default; --non-interactive skips prompts
        // (auto-skips Claude hooks when stdout is not a TTY).
        "bd" => ("bd", &["init", "--non-interactive"]),
        // openspec init scaffolds openspec/ and prompts for AI-tool config;
        // --tools none skips that prompt.
        "openspec" => ("openspec", &["init", "--tools", "none"]),
        "specodelic" => ("specodelic", &["doctor"]),
        _ => return None,
    };
    Some((cmd, args.to_vec()))
}

/// `dir` prepended to an inherited PATH (so freshly downloaded binaries in
/// `~/.ddl/bin` can be initialized and find their own siblings).
pub fn prepend_path(
    dir: &std::path::Path,
    existing: Option<&std::ffi::OsStr>,
) -> std::ffi::OsString {
    let mut paths = vec![dir.to_path_buf()];
    if let Some(rest) = existing
        .filter(|p| !p.is_empty())
        .map(std::env::split_paths)
    {
        paths.extend(rest);
    }
    std::env::join_paths(paths).unwrap_or_else(|_| dir.as_os_str().to_os_string())
}

/// Human-readable resume lines for failed init steps: what failed and the
/// exact command(s) to finish manually (or via a selective re-run).
pub fn resume_summary(failures: &[InitFailure]) -> Vec<String> {
    failures
        .iter()
        .map(|f| {
            let (cmd, args) =
                init_command(&f.tool, false).unwrap_or((f.tool.as_str(), Vec::new()));
            let manual = args.iter().fold(cmd.to_string(), |acc, a| format!("{acc} {a}"));
            format!(
                "  ✗ {} init failed: {}\n      finish with: {}\n      or re-run: ddl init --tools {}",
                f.tool, f.detail, manual, f.tool
            )
        })
        .collect()
}

/// Run a tool's init command after installation.
pub fn run_tool_init(tool: &Tool, verbose: bool, non_interactive: bool) -> Result<()> {
    let Some((cmd, args)) = init_command(tool.name, non_interactive) else {
        return Ok(()); // tool has no init command — skip silently
    };

    // Check if tool is on PATH; if not, probe its .ddl/bin destination
    // directly (DDL-6zn.3: freshly downloaded binaries are not on PATH yet —
    // the old code skipped their init with only a hint).
    let on_path = is_tool_installed(cmd);
    let at_dest = Platform::detect().map(|p| binary_dest_path(tool, &p));
    let at_dest_exists = at_dest.as_ref().is_some_and(|p| p.is_file());
    if !on_path && !at_dest_exists {
        eprintln!("  ⚠ {} not found on PATH — skipping init", cmd);
        return Ok(());
    }

    verbose_print(verbose, &format!("running: {} init", cmd));

    let mut command = if on_path {
        Command::new(cmd)
    } else {
        Command::new(at_dest.expect("checked at_dest_exists above"))
    };
    command.args(&args);
    // Prepend the ddl bin dir so downloaded tools see their siblings.
    if let Some(bin_dir) = ddl_bin_dir() {
        command.env(
            "PATH",
            prepend_path(&bin_dir, std::env::var_os("PATH").as_deref()),
        );
    }

    let output = command
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
            "  ✗ {} init exited with code {}: {}",
            tool.name,
            output.status.code().unwrap_or(-1),
            stderr.lines().next().unwrap_or("unknown error")
        );
        // DDL-6zn.3: init failures are no longer advisory warnings — the
        // cascade collects them and the run exits with a resume summary,
        // instead of silently leaving the repo half-configured.
        Err(DdlError::InstallFailed(format!(
            "{} init exited with code {}: {}",
            tool.name,
            output.status.code().unwrap_or(-1),
            stderr.lines().next().unwrap_or("unknown error")
        )))
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
                        (InstallMethod::Cargo, upgrade_cargo(tool, verbose))
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
            let result = execute_upgrade(&other, tool, platform, verbose);
            (other, result)
        }
    };

    let new_version = resolve_with_dest(tool.name, Some(&binary_dest_path(tool, platform)));

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
    // Scoped packages (e.g. @fission-ai/openspec) — use npm_package.
    let package = tool.npm_package.unwrap_or(tool.crate_name);
    verbose_print(
        verbose,
        &format!("running: npm install -g {package}@latest"),
    );
    let status = npm_command("npm")
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_version_line_variants() {
        assert_eq!(parse_version_line("ddl", "ddl 0.6.0"), Some("0.6.0".into()));
        assert_eq!(
            parse_version_line("bd", "bd version 1.3.0"),
            Some("1.3.0".into())
        );
        assert_eq!(parse_version_line("wai", "v1.2.3"), Some("1.2.3".into()));
        assert_eq!(
            parse_version_line("x", "single-token"),
            Some("single-token".into())
        );
    }

    // Unix-only: the stub is a shebang script; on Windows the probe would
    // need an .exe stub, which the executor shims differently.
    #[cfg(unix)]
    #[test]
    fn test_probe_version_at_finds_version_in_ddl_bin() {
        // DDL-x0m: after a binary download to .ddl/bin (not yet on PATH),
        // the probe must run the binary at its destination directly.
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("fake-tool");
        std::fs::write(&script, "#!/bin/sh\necho \"fake-tool version 9.9.9\"\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        assert_eq!(
            probe_version_at(&script, "fake-tool"),
            Some("9.9.9".to_string())
        );
        assert_eq!(
            probe_version_at(dir.path().join("nope").as_path(), "fake-tool"),
            None
        );
    }

    // --- DDL-6zn.3: init-cascade semantics ---------------------------------

    #[test]
    fn init_command_per_tool_and_mode() {
        // dont runs `init` (creates project state) — the old `prime --plain`
        // failed circularly: no .dont/ project exists until dont init runs.
        assert_eq!(init_command("dont", false), Some(("dont", vec!["init"])));
        // pretender's wizard prompt leaked into -y output — non-interactive
        // mode must pass --non-interactive (best-guess defaults).
        assert_eq!(
            init_command("pretender", true),
            Some(("pretender", vec!["init", "--non-interactive"]))
        );
        assert_eq!(
            init_command("pretender", false),
            Some(("pretender", vec!["init"]))
        );
        // openspec: skip the AI-tool config prompt non-interactively.
        assert_eq!(
            init_command("openspec", true),
            Some(("openspec", vec!["init", "--tools", "none"]))
        );
        // tools without an init command are skipped silently (fotos-mcp's
        // 'expect initialize request' failure class).
        assert_eq!(init_command("fotos-mcp", false), None);
        assert_eq!(init_command("turu", true), None);
        assert_eq!(init_command("incitaciones", false), None);
        assert_eq!(init_command("unknown-tool", false), None);
    }

    #[test]
    fn prepend_path_puts_ddl_bin_first() {
        use std::ffi::OsString;
        let bin = std::path::PathBuf::from("/home/t/.ddl/bin");
        let expected: OsString = std::env::join_paths(["/home/t/.ddl/bin", "/usr/bin"]).unwrap();
        assert_eq!(
            prepend_path(&bin, Some(&OsString::from("/usr/bin"))),
            expected
        );
        assert_eq!(prepend_path(&bin, None), OsString::from("/home/t/.ddl/bin"));
        assert_eq!(
            prepend_path(&bin, Some(&OsString::new())),
            OsString::from("/home/t/.ddl/bin")
        );
    }

    #[test]
    fn resume_summary_lists_exact_finish_commands() {
        let failures = vec![
            InitFailure {
                tool: "dont".into(),
                detail: "exited with code 1".into(),
            },
            InitFailure {
                tool: "ah".into(),
                detail: "openspec/ directory not found".into(),
            },
        ];
        let summary = resume_summary(&failures).join("\n");
        assert!(
            summary.contains("dont init"),
            "missing tool's own init cmd: {summary}"
        );
        assert!(
            summary.contains("ddl init --tools dont"),
            "missing re-run cmd: {summary}"
        );
        assert!(summary.contains("ah init"));
        assert!(summary.contains("openspec/ directory not found"));
    }

    #[test]
    fn test_binary_dest_dir_pure() {
        use crate::platform::Os;
        let home = std::path::PathBuf::from("/home/tester");
        assert_eq!(
            binary_dest_dir(&Os::Linux, Some(home.as_path()), None),
            std::path::PathBuf::from("/home/tester/.ddl/bin")
        );
        assert_eq!(
            binary_dest_dir(&Os::Macos, Some(home.as_path()), None),
            std::path::PathBuf::from("/home/tester/.ddl/bin")
        );
        let lad = std::path::PathBuf::from("C:\\Users\\u\\AppData\\Local");
        assert_eq!(
            binary_dest_dir(&Os::Windows, None, Some(lad.as_path())),
            lad.join("ddl").join("bin")
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_resolve_version_falls_back_to_dest_when_not_on_path() {
        use std::os::unix::fs::PermissionsExt;
        let temp = tempfile::tempdir().unwrap();
        let dest_dir = temp.path().join(".ddl").join("bin");
        std::fs::create_dir_all(&dest_dir).unwrap();
        let script = dest_dir.join("faketool");
        std::fs::write(&script, "#!/bin/sh\necho \"faketool version 1.2.3\"\n").unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

        // The binary is not on PATH — the dest fallback must still find it.
        assert_eq!(
            resolve_with_dest("faketool", Some(&dest_dir)),
            Some("1.2.3".to_string())
        );
    }

    #[test]
    fn test_resolve_with_dest_none_when_nothing_installed() {
        // A name that exists nowhere on PATH and in a dest dir without it.
        let empty = tempfile::tempdir().unwrap();
        assert_eq!(
            resolve_with_dest("definitely-not-real-xyz", Some(empty.path())),
            None
        );
    }

    /// Verify verbose_print exists and accepts both values without panicking.
    /// The actual stderr output is verified by integration tests that run
    /// the ddl binary with --verbose.
    #[test]
    fn test_verbose_print_accepts_bool() {
        // Must not panic for either value
        verbose_print(true, "trace message");
        verbose_print(false, "trace message");
    }

    #[test]
    fn test_sha256_hex_known_digests() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"hello world"),
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[test]
    fn test_parse_checksums_shasum_format() {
        let content = concat!(
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855  wai_0.1.0_darwin_arm64.tar.gz\n",
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9  beads_1.0.0_linux_amd64.zip\n"
        );
        let map = parse_checksums(content).expect("should parse two lines");
        assert_eq!(
            map.get("wai_0.1.0_darwin_arm64.tar.gz").unwrap(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            map.get("beads_1.0.0_linux_amd64.zip").unwrap(),
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[test]
    fn test_parse_checksums_single_space_separator() {
        let content =
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9 wai.tar.gz\n";
        let map = parse_checksums(content).expect("single-space lines parse");
        assert_eq!(
            map.get("wai.tar.gz").unwrap(),
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[test]
    fn test_parse_checksums_binary_mode_star_prefix() {
        // `sha256sum -b` emits `hex *filename` (single space, asterisk prefix)
        let content =
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9 *wai.tar.gz\n";
        let map = parse_checksums(content).expect("binary-mode line parses");
        assert_eq!(
            map.get("wai.tar.gz").unwrap(),
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[test]
    fn test_parse_checksums_skips_malformed_lines() {
        let content = concat!(
            "not-a-checksum oops.tar.gz\n",
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9  good.tar.gz\n",
            "\n",
            "\"b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9\" *quoted.tar.gz\n"
        );
        let map = parse_checksums(content).expect("at least one valid line");
        assert_eq!(map.len(), 2);
        assert!(map.contains_key("good.tar.gz"));
        assert!(
            map.contains_key("quoted.tar.gz"),
            "bsd-style quoted line parses"
        );
    }

    #[test]
    fn test_parse_checksums_no_valid_lines_returns_none() {
        assert_eq!(parse_checksums(""), None);
        assert_eq!(parse_checksums("garbage\nlines\n"), None);
    }

    #[test]
    fn test_verify_checksum_accepts_matching_digest() {
        let digest = sha256_hex(b"payload");
        assert!(verify_checksum(b"payload", &digest, "wai", "https://example.test").is_ok());
    }

    #[test]
    fn test_verify_checksum_aborts_on_mismatch_naming_tool() {
        let digest = sha256_hex(b"payload");
        let err = verify_checksum(
            b"tampered",
            &digest,
            "wai",
            "https://example.test/asset.tar.gz",
        )
        .expect_err("mismatch must error");
        match &err {
            DdlError::ChecksumMismatch { tool, .. } => assert_eq!(tool, "wai"),
            other => panic!("expected ChecksumMismatch, got: {other}"),
        }
        // Error text names the tool and both digests so the operator can triage
        let text = err.to_string();
        assert!(text.contains("wai"));
        assert!(text.contains(&digest));
    }

    #[test]
    fn test_verify_checksum_accepts_uppercase_expected() {
        // Some release pipelines emit uppercase hex — normalize before comparing
        let digest = sha256_hex(b"payload").to_uppercase();
        assert!(verify_checksum(b"payload", &digest, "wai", "https://example.test").is_ok());
    }

    /// github_token reads GITHUB_TOKEN, then GH_TOKEN, and ignores empty
    /// values. Env vars are process-global, so the test serializes access
    /// itself and restores prior values afterwards.
    #[test]
    fn test_github_token_reads_env() {
        // SAFETY: env var manipulation in a single-threaded test; values are
        // saved and restored so other tests see the original environment.
        unsafe {
            let saved_gh = std::env::var("GITHUB_TOKEN").ok();
            let saved_github = std::env::var("GH_TOKEN").ok();

            std::env::remove_var("GITHUB_TOKEN");
            std::env::remove_var("GH_TOKEN");
            assert_eq!(github_token(), None);

            std::env::set_var("GITHUB_TOKEN", "tok-1");
            assert_eq!(github_token(), Some("tok-1".to_string()));

            // Empty GITHUB_TOKEN is ignored, falls through to GH_TOKEN
            std::env::set_var("GITHUB_TOKEN", "");
            std::env::set_var("GH_TOKEN", "tok-2");
            assert_eq!(github_token(), Some("tok-2".to_string()));

            match saved_gh {
                Some(v) => std::env::set_var("GITHUB_TOKEN", v),
                None => std::env::remove_var("GITHUB_TOKEN"),
            }
            match saved_github {
                Some(v) => std::env::set_var("GH_TOKEN", v),
                None => std::env::remove_var("GH_TOKEN"),
            }
        }
    }
}
