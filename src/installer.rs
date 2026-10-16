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
//!
//! Rationale (DDL-yts, SEC-03): verification must be *available*, not merely
//! attempted. A release that publishes no usable checksum — missing,
//! malformed, incomplete, or transport-failed metadata — cannot be
//! authenticated, so the download is REJECTED before any destination
//! mutation, instead of being installed with a verbose-only skip warning.
//! verified* installs. Provenance verification (DDL-wof, SEC-03/06) binds a
//! release to its publisher/build identity: the release workflow publishes a
//! `provenance.json` manifest naming the repository, workflow and tag that
//! built each asset, and the installer rejects artifacts whose manifest names
//! an unexpected repository/workflow/tag or whose bytes diverge from the
//! manifest's per-asset digest. The trust roots are the constants
//! `EXPECTED_PROVENANCE_REPOSITORY` / `EXPECTED_PROVENANCE_WORKFLOW`; the
//! legacy transition (`DDL_ALLOW_UNVERIFIED_RELEASE=1`) may authorize a
//! *missing* manifest but never a *contradicting* one.
//!
//! Rationale (DDL-uu8, SEC-04): children spawned by ddl (cargo, npm, npx,
//! tool-init commands) must not silently inherit the caller's full environment
//! and filesystem authority. Every such child runs with an explicit
//! restricted-privilege policy: an allowlisted environment (credentials and
//! other non-approved variables are dropped), and execution scopes whose
//! write roots reach beyond this project (global skill installs) are gated
//! behind explicit authorization env vars. Enforcement mechanism is
//! deliberately cheap and portable — controlled env dictionary + explicit
//! scope authorization, fail-closed on unsupported platforms — with NO silent
//! unsandboxed fallback; kernel-level sandboxing would require a verifier
//! dependency, which the ticket explicitly reserves for a separate design
//! approval.
//!
//! Rationale (DDL-4oi, SEC-01): archive extraction must never write directly
//! to the installed destination. The expected binary is extracted as the only
//! accepted entry — regular files only (links and special files rejected),
//! size-bounded, traversal-safe — into an *exclusive* staging directory on
//! the destination's filesystem, then atomically swapped into place. On any
//! failure before the swap the working executable remains byte-for-byte
//! intact and no staging residue is left behind. POSIX uses rename(2) (fully
//! atomic); Windows uses a move-aside + rollback protocol with the DDL-5bs
//! install lock serializing concurrent upgrades.
//!
//! Rationale (DDL-5bs, SEC-04): installs used to float — binary installs
//! resolved `releases/latest`, cargo had no `--version`/`--locked`, npm was
//! `@latest`. Replay now resolves the artifact purely from the recorded lock
//! identity (`manifest::InstallLock`): the replay path has no latest input at
//! all, so two locked replays select identical versions, URLs and digests
//! even when latest changes. Only explicit upgrades resolve latest, and they
//! must clear the configurable release-age policy (deterministic `*_epoch_day`
//! inputs, no wall clock) before the lock is updated atomically.
//!
//! Rationale (DDL-1z1, SEC-02): bootstrap consumes finite host resources —
//! release bodies, extracted files, captured child output and wall-clock
//! time — every one of which used to be unbounded. Each is now governed by an
//! explicit numeric budget ([`MAX_RELEASE_ASSET_BYTES`],
//! [`MAX_CHILD_OUTPUT_BYTES`], [`child_deadline`]) enforced fail-closed: an
//! exhausted budget aborts with diagnostics naming the budget, never degrades
//! into a less restricted install method, and leaves the previous
//! installation intact. Child processes spawned during bootstrap run in their
//! own process group so a deadline kill sweeps descendants too.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Print a trace message to stderr when `verbose` is enabled.
/// Used to show what the installer is doing without cluttering normal output.
fn verbose_print(verbose: bool, msg: &str) {
    if verbose {
        eprintln!("  · {msg}");
    }
}

use crate::dot_ddl::DdlDir;
use crate::error::{DdlError, Result};
use crate::manifest::{InstallLock, LockEntry, Manifest, ToolEntry};
use crate::platform::{MANAGED_TOOLS, Os, PackageManager, Platform, Tool};

/// Verification outcome of a binary release download (DDL-yts).
///
/// `label()` is the machine-readable value used in JSON output events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerificationStatus {
    /// sha256 matched the checksum published for the release.
    Verified,
    /// Installed without release-checksum verification, explicitly authorized
    /// by the user via `DDL_ALLOW_UNVERIFIED_RELEASE=1` (legacy transition).
    UnverifiedAuthorized,
    /// Installed without release-checksum verification: non-binary method
    /// (cargo/npm rely on their registries' own integrity guarantees).
    Unverified,
}

impl VerificationStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Verified => "verified",
            Self::UnverifiedAuthorized => "unverified-authorized",
            Self::Unverified => "unverified",
        }
    }

    /// Human-readable suffix appended to install result messages so human and
    /// JSON output both carry the distinction.
    pub fn suffix(self) -> String {
        match self {
            Self::Verified => " (sha256 verified)".to_string(),
            Self::UnverifiedAuthorized => {
                " (unverified — explicitly authorized via DDL_ALLOW_UNVERIFIED_RELEASE)".to_string()
            }
            Self::Unverified => String::new(),
        }
    }
}

/// Result of looking up the published checksum for a release asset (DDL-yts).
///
/// Replaces the old `Option<String>`: every flavor of "verification is not
/// possible" now carries its reason so the rejection diagnostic is accurate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChecksumLookup {
    /// A published sha256 digest for the asset.
    Published(String),
    /// No usable checksum metadata: missing, malformed, incomplete (asset
    /// absent from the manifest), or the fetch failed. The string is the
    /// human-readable reason used in rejection diagnostics.
    Unavailable(&'static str),
}

/// Replay policy (DDL-5bs, SEC-04): resolve the artifact purely from the
/// recorded lock identity.
///
/// The signature takes the lock and the tool name ONLY — there is no
/// latest-version input, so the replay path cannot resolve latest without an
/// API change. The returned identity feeds the existing verification policy
/// unchanged (DDL-yts checksum / DDL-wof provenance still run on the fetched
/// bytes).
pub fn replay_decision<'a>(lock: &'a InstallLock, tool: &str) -> Result<&'a LockEntry> {
    lock.get(tool).ok_or_else(|| {
        DdlError::InstallFailed(format!(
            "no recorded install identity for {tool}; replay resolves only recorded identities — run `ddl install {tool}` to record one"
        ))
    })
}

/// Replay command for a Cargo-installed tool: exact version, locked
/// dependency graph (DDL-5bs). Never a floating or latest reference.
pub fn cargo_replay_args(crate_name: &str, version: &str) -> Vec<String> {
    vec![
        "install".into(),
        crate_name.into(),
        "--version".into(),
        version.into(),
        "--locked".into(),
    ]
}

/// Replay command for an npm-installed tool: the selected package version,
/// never `@latest` (DDL-5bs).
pub fn npm_replay_args(package: &str, version: &str) -> Vec<String> {
    vec![
        "install".into(),
        "-g".into(),
        format!("{package}@{version}"),
    ]
}

/// A latest release resolved by an EXPLICIT upgrade (DDL-5bs).
///
/// Unlike the replay path, upgrades are the only place latest resolution
/// happens, and the candidate carries its publication day so the release-age
/// policy is a deterministic function of explicit time inputs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseCandidate {
    pub version: String,
    pub artifact_url: String,
    pub sha256: String,
    pub published_epoch_day: u64,
}

/// Outcome of an explicit upgrade decision (DDL-5bs).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpgradeDecision {
    /// The candidate is identical to the recorded identity.
    UpToDate,
    /// The candidate is younger than the configured minimum release age; the
    /// lock is NOT updated. `reason` names the policy and the age shortfall.
    Deferred { reason: String },
    /// The resolved change, old identity → new identity. The caller presents
    /// it and persists the new entry via `InstallLock::save` (atomic).
    Upgrade { from: LockEntry, to: LockEntry },
}

/// Decide an explicit upgrade against the release-age policy (DDL-5bs).
///
/// `min_release_age_days` is the configurable supply-chain cool-down;
/// `today_epoch_day` is the deterministic time input (epoch days, UTC).
pub fn upgrade_decision(
    current: &LockEntry,
    candidate: &ReleaseCandidate,
    min_release_age_days: u64,
    today_epoch_day: u64,
) -> Result<UpgradeDecision> {
    if candidate.version == current.version
        && candidate.artifact_url == current.artifact_url
        && candidate.sha256 == current.sha256
    {
        return Ok(UpgradeDecision::UpToDate);
    }
    let age = today_epoch_day.saturating_sub(candidate.published_epoch_day);
    if age < min_release_age_days {
        return Ok(UpgradeDecision::Deferred {
            reason: format!(
                "release {} is {age} day(s) old; the release-age policy requires at least {min_release_age_days} day(s)",
                candidate.version
            ),
        });
    }
    Ok(UpgradeDecision::Upgrade {
        from: current.clone(),
        to: LockEntry {
            tool: current.tool.clone(),
            version: candidate.version.clone(),
            artifact_url: candidate.artifact_url.clone(),
            sha256: candidate.sha256.clone(),
            recorded_epoch_day: today_epoch_day,
        },
    })
}

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

/// DDL-yts policy for release downloads, as a pure decision (no network, no
/// filesystem) so it can be regression-tested without connectivity.
///
/// - `ChecksumLookup::Published(digest)` → sha256 must match: `Ok(Verified)`
///   or `Err(ChecksumMismatch)`. A mismatch is NEVER bypassed by
///   `allow_unverified` and never falls back to cargo.
/// - `ChecksumLookup::Unavailable(reason)` → `Err(ChecksumUnavailable)` by
///   default (secure default: reject unverified downloads before any
///   destination mutation). Only when the user explicitly authorized the
///   legacy-unverified transition (`DDL_ALLOW_UNVERIFIED_RELEASE=1`) does it
///   return `Ok(UnverifiedAuthorized)`.
pub fn enforce_checksum_policy(
    tool: &str,
    url: &str,
    lookup: ChecksumLookup,
    bytes: &[u8],
    allow_unverified: bool,
) -> Result<VerificationStatus> {
    match lookup {
        ChecksumLookup::Published(expected) => {
            verify_checksum(bytes, &expected, tool, url)?;
            Ok(VerificationStatus::Verified)
        }
        ChecksumLookup::Unavailable(reason) => {
            if allow_unverified {
                Ok(VerificationStatus::UnverifiedAuthorized)
            } else {
                Err(DdlError::ChecksumUnavailable {
                    tool: tool.to_string(),
                    url: url.to_string(),
                    reason: reason.to_string(),
                })
            }
        }
    }
}

/// True only when the user explicitly authorized a legacy-unverified install.
fn unverified_authorized() -> bool {
    matches!(
        std::env::var("DDL_ALLOW_UNVERIFIED_RELEASE").as_deref(),
        Ok("1" | "true" | "yes")
    )
}

// -------------------------------------------------------------------------
// DDL-wof (SEC-03/06): publisher / build-identity verification.
// -------------------------------------------------------------------------

/// Trust root: the GitHub repository whose release workflow is allowed to
/// publish ddl release artifacts (DDL-wof).
pub const EXPECTED_PROVENANCE_REPOSITORY: &str = "charly-vibes/dulce-de-leche";

/// Trust root: the workflow name that is allowed to build ddl release
/// artifacts (DDL-wof). Matches `.github/workflows/release.yml`.
pub const EXPECTED_PROVENANCE_WORKFLOW: &str = "Release";

/// The provenance manifest a release publishes next to its assets (DDL-wof).
/// Generated by the release workflow; binds each artifact's digest to the
/// repository/workflow/tag identity that produced the release.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
pub struct ProvenanceManifest {
    /// GitHub repository that built and published the release.
    pub repository: String,
    /// Name of the workflow that built the release.
    pub workflow: String,
    /// Tag the release was published under.
    pub tag: String,
    /// sha256 digest per artifact filename (lowercase hex).
    #[serde(default)]
    pub artifacts: std::collections::HashMap<String, String>,
}

/// Result of looking up the release's provenance manifest (DDL-wof).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProvenanceLookup {
    /// The release publishes a well-formed provenance manifest.
    Published(ProvenanceManifest),
    /// No usable provenance manifest: missing, malformed, unreadable, or the
    /// fetch failed. The string is the human-readable reason used in
    /// rejection diagnostics.
    Unavailable(&'static str),
}

/// DDL-wof policy for release build identity, as a pure decision (no network,
/// no filesystem) so it can be regression-tested without connectivity.
///
/// - Identity checks are NEVER bypassed: a published manifest naming an
///   unexpected repository, workflow, or tag — or listing a digest that does
///   not match the downloaded bytes — is rejected even when the user
///   authorized unverified installs. Authorization only bridges a *missing*
///   manifest (like DDL-yts) or an archive URL with no tag to check against.
/// - A manifest that fails identity or digest binding yields
///   `ChecksumMismatch` for digest divergence (bytes altered) and
///   `ChecksumUnavailable` for identity/availability violations — reusing the
///   SEC-03 rejection plumbing so no destination mutation can happen.
pub fn enforce_provenance_policy(
    tool: &str,
    url: &str,
    expected_tag: Option<&str>,
    lookup: ProvenanceLookup,
    bytes: &[u8],
    allow_unverified: bool,
) -> Result<VerificationStatus> {
    let reject = |reason: String| DdlError::ChecksumUnavailable {
        tool: tool.to_string(),
        url: url.to_string(),
        reason,
    };

    let Some(tag) = expected_tag else {
        if allow_unverified {
            return Ok(VerificationStatus::UnverifiedAuthorized);
        }
        return Err(reject(
            "archive URL does not carry a release tag, so the provenance identity (repository/workflow/tag) cannot be checked".to_string(),
        ));
    };

    let manifest = match lookup {
        ProvenanceLookup::Unavailable(reason) => {
            if allow_unverified {
                return Ok(VerificationStatus::UnverifiedAuthorized);
            }
            return Err(reject(format!("no usable provenance manifest: {reason}")));
        }
        ProvenanceLookup::Published(manifest) => manifest,
    };

    if manifest.repository != EXPECTED_PROVENANCE_REPOSITORY {
        return Err(reject(format!(
            "provenance manifest declares repository `{}`, expected `{}`",
            manifest.repository, EXPECTED_PROVENANCE_REPOSITORY
        )));
    }
    if manifest.workflow != EXPECTED_PROVENANCE_WORKFLOW {
        return Err(reject(format!(
            "provenance manifest declares workflow `{}`, expected `{}`",
            manifest.workflow, EXPECTED_PROVENANCE_WORKFLOW
        )));
    }
    if manifest.tag != tag {
        return Err(reject(format!(
            "provenance manifest describes tag `{}`, but the asset was fetched from release `{tag}`",
            manifest.tag
        )));
    }

    let asset = url.rsplit_once('/').map(|(_, a)| a).unwrap_or(url);
    match manifest.artifacts.get(asset) {
        None => Err(reject(format!(
            "provenance manifest does not list asset `{asset}`"
        ))),
        Some(expected) => {
            verify_checksum(bytes, expected, tool, url)?;
            Ok(VerificationStatus::Verified)
        }
    }
}

/// Extract the release tag from a GitHub release download URL
/// (`…/releases/download/<tag>/<asset>`). `None` when the URL does not carry
/// one — provenance identity cannot be checked against a non-release URL.
fn release_tag_from_url(archive_url: &str) -> Option<&str> {
    let rest = archive_url.split_once("releases/download/")?.1;
    rest.split('/').next().filter(|t| !t.is_empty())
}

/// Fetch the release's provenance manifest (`provenance.json` next to the
/// asset, DDL-wof). Any flavor of "verification is not possible" — missing
/// asset, malformed JSON, unreadable body, transport failure — is reported as
/// [`ProvenanceLookup::Unavailable`] with the precise reason; the policy in
/// `enforce_provenance_policy` decides rejection, not this lookup.
fn fetch_provenance(archive_url: &str, _verbose: bool) -> ProvenanceLookup {
    let Some((base, _)) = archive_url.rsplit_once('/') else {
        return ProvenanceLookup::Unavailable("asset URL has no parent path");
    };
    let provenance_url = format!("{base}/provenance.json");
    match reqwest::blocking::get(&provenance_url) {
        Ok(resp) if resp.status().is_success() => match resp.bytes() {
            Ok(body) => match serde_json::from_slice::<ProvenanceManifest>(&body) {
                Ok(manifest) => ProvenanceLookup::Published(manifest),
                Err(_) => ProvenanceLookup::Unavailable("provenance.json is malformed"),
            },
            Err(_) => ProvenanceLookup::Unavailable("provenance.json body could not be read"),
        },
        // A non-success (404, 403, …) is indistinguishable from "no manifest
        // published" and must not degrade to skip-and-install — the policy
        // rejects unless explicitly authorized.
        Ok(_) => ProvenanceLookup::Unavailable("release publishes no provenance.json asset"),
        Err(_) => ProvenanceLookup::Unavailable("provenance.json could not be fetched"),
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

    let (method, result) = match method {
        InstallMethod::Binary => match install_binary(tool, platform, verbose) {
            Ok(status) => (InstallMethod::Binary, Ok(status)),
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
        },
        other => {
            let result = execute_install(&other, tool, platform, verbose)
                .map(|()| VerificationStatus::Unverified);
            (other, result)
        }
    };

    let detected = resolve_with_dest(tool.name, Some(&binary_dest_path(tool, platform)))
        .unwrap_or_else(|| "unknown".to_string());

    match result {
        Ok(status) => {
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
                message: format!(
                    "{} v{} installed via {}{}",
                    tool.name,
                    detected,
                    method,
                    status.suffix()
                ),
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
fn execute_upgrade(
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
fn install_binary(tool: &Tool, platform: &Platform, verbose: bool) -> Result<VerificationStatus> {
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

/// Look up the published sha256 for the asset at `archive_url`, reporting the
/// exact reason whenever verification is not possible (DDL-yts).
///
/// Conventions checked in order:
/// 1. `checksums.txt` next to the asset (shasum format — ddl's own release
///    workflow publishes this), with the archive looked up by filename
/// 2. a per-asset `<archive>.sha256` file containing a bare digest
///
/// Returns [`ChecksumLookup::Unavailable`] — with the precise reason — when
/// the release publishes no usable checksum source, the metadata is
/// malformed, the asset is absent from the manifest, or the metadata fetch
/// itself fails. Missing/malformed/transport-failed are all rejections under
/// the DDL-yts policy (see `enforce_checksum_policy`), not skip-and-install.
fn fetch_expected_checksum(archive_url: &str, _verbose: bool) -> ChecksumLookup {
    let Some((base, archive_name)) = archive_url.rsplit_once('/') else {
        return ChecksumLookup::Unavailable("asset URL has no parent path");
    };

    let checksums_url = format!("{base}/checksums.txt");
    match reqwest::blocking::get(&checksums_url) {
        Ok(resp) if resp.status().is_success() => {
            if let Ok(body) = resp.text() {
                match parse_checksums(&body) {
                    Some(map) => {
                        if let Some(expected) = map.get(archive_name).cloned() {
                            return ChecksumLookup::Published(expected);
                        }
                        return ChecksumLookup::Unavailable(
                            "asset is not listed in this release's checksums.txt",
                        );
                    }
                    None => {
                        return ChecksumLookup::Unavailable("checksums.txt is malformed");
                    }
                }
            }
            return ChecksumLookup::Unavailable("checksums.txt body could not be read");
        }
        Ok(_) => {
            // checksums.txt answered but not with success — fall through to
            // the per-asset convention.
        }
        Err(_) => {
            // Metadata transport failed — an attacker or outage must not be
            // indistinguishable from "no checksum published". Rejected.
            return ChecksumLookup::Unavailable("checksums.txt could not be fetched");
        }
    }

    let per_asset_url = format!("{archive_url}.sha256");
    if let Ok(resp) = reqwest::blocking::get(&per_asset_url)
        && resp.status().is_success()
        && let Ok(body) = resp.text()
    {
        let digest = body.split_whitespace().next().unwrap_or("").to_lowercase();
        if digest.len() == 64 && digest.chars().all(|c| c.is_ascii_hexdigit()) {
            return ChecksumLookup::Published(digest);
        }
        return ChecksumLookup::Unavailable("per-asset .sha256 file is malformed");
    }

    ChecksumLookup::Unavailable("release publishes no checksum for the asset")
}

/// Verify a downloaded release archive under the DDL-yts policy.
///
/// Missing / malformed / incomplete / transport-failed checksum metadata is a
/// rejection (see `enforce_checksum_policy`); the only bypass is the explicit
/// `DDL_ALLOW_UNVERIFIED_RELEASE=1` authorization, which yields
/// [`VerificationStatus::UnverifiedAuthorized`]. Rejection happens before any
/// destination mutation — the caller must not extract when this returns Err.
fn verify_download(
    archive_url: &str,
    archive_bytes: &[u8],
    tool: &Tool,
    verbose: bool,
) -> Result<VerificationStatus> {
    let status = enforce_checksum_policy(
        tool.name,
        archive_url,
        fetch_expected_checksum(archive_url, verbose),
        archive_bytes,
        unverified_authorized(),
    )?;
    // DDL-wof: identity must match too — checksum alone authenticates bytes
    // against *some* published digest, provenance authenticates the publisher
    // and build identity behind that digest.
    enforce_provenance_policy(
        tool.name,
        archive_url,
        release_tag_from_url(archive_url),
        fetch_provenance(archive_url, verbose),
        archive_bytes,
        unverified_authorized(),
    )?;
    Ok(status)
}

/// Fetch a release archive, mapping the release-404
/// case to `NoReleaseBinary` and any other non-success to `InstallFailed`.
fn fetch_release_asset(url: &str, tool_name: &str) -> Result<bytes::Bytes> {
    use std::io::Read;
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
    // DDL-1z1: the release body is bounded by MAX_RELEASE_ASSET_BYTES. The
    // declared Content-Length is rejected before the first byte is read; the
    // streamed byte count is re-checked as chunks arrive (covers the
    // unknown-length case). The body is never buffered past the budget.
    enforce_download_budget(response.content_length(), 0)?;
    let mut response = response;
    let mut body: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 64 * 1024];
    // `blocking::Response` implements `std::io::Read` (decoded bytes), so the
    // streamed count is re-checked as data arrives — the unknown-length case
    // is still bounded.
    loop {
        let n = response
            .read(&mut chunk)
            .map_err(|e| DdlError::InstallFailed(format!("Download failed: {e} for {url}")))?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..n]);
        enforce_download_budget(None, body.len() as u64)?;
    }
    Ok(body.into())
}

/// Upper bound on the size of an extracted binary (DDL-4oi). The declared
/// entry size is checked before any byte is written, so a hostile or
/// corrupted archive cannot exhaust the destination disk with staging data.
pub const MAX_BINARY_BYTES: u64 = 512 * 1024 * 1024;

/// Upper bound on the size of a downloaded release asset (DDL-1z1, SEC-02).
/// The declared Content-Length is checked before the first byte is read, and
/// the streamed byte count is re-checked as chunks arrive, so a hostile or
/// corrupted release body cannot exhaust host memory or disk before the
/// policy rejects it.
pub const MAX_RELEASE_ASSET_BYTES: u64 = 512 * 1024 * 1024;

/// Upper bound on output captured from a bootstrap child process
/// (DDL-1z1, SEC-02). Version probes and init commands cannot flood ddl's
/// memory with unbounded output; bytes past the budget are discarded and the
/// child is killed with the budget named in the error.
pub const MAX_CHILD_OUTPUT_BYTES: usize = 4 * 1024 * 1024;

/// Default child runtime deadline in seconds (DDL-1z1, SEC-02). A child that
/// has not exited by the deadline — nor have its descendants — is killed.
/// Controlled fixtures may shorten the deadline via `DDL_CHILD_DEADLINE_SECS`
/// (1–3600); the env override exists for tests, not as a production escape
/// hatch (any value outside the accepted range falls back to the default).
pub const DEFAULT_CHILD_DEADLINE_SECS: u64 = 30;

/// Budget for a single tool's init child. Most tool inits are quick; `bd`
/// bootstrap-creates an embedded Dolt database on first run, which can take
/// well over the default budget on a loaded CI runner — killing it mid-init
/// would leave the store half-created and fail every `ddl init` that touches
/// bd (observed 2026-10-14: CI runner killed bd init at the default 30s).
fn init_deadline_for(tool_name: &str) -> std::time::Duration {
    if tool_name == "bd" {
        std::time::Duration::from_secs(300)
    } else {
        child_deadline()
    }
}

/// Resolve the configured child runtime deadline (DDL-1z1).
pub fn child_deadline() -> std::time::Duration {
    use std::time::Duration;
    std::env::var("DDL_CHILD_DEADLINE_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .filter(|secs| (1..=3600).contains(secs))
        .map(Duration::from_secs)
        .unwrap_or(Duration::from_secs(DEFAULT_CHILD_DEADLINE_SECS))
}

/// Build a budget-exhaustion error naming the exhausted budget (DDL-1z1).
/// The message is what reaches stderr and the JSON error envelope, so the
/// budget name must stay in the text.
fn budget_exceeded(budget: &str, detail: String) -> DdlError {
    DdlError::InstallFailed(format!("budget exceeded: {budget} — {detail}"))
}

/// DDL-1z1 download budget for release assets, as a pure decision (no
/// network, no I/O): rejects a declared Content-Length over
/// [`MAX_RELEASE_ASSET_BYTES`] *before* any byte is fetched, and a streamed
/// byte count over the budget as chunks arrive (also covers the unknown-length
/// case, where the header cannot pre-check). Fail-closed: there is no
/// authorization that permits exceeding the budget.
pub fn enforce_download_budget(content_length: Option<u64>, streamed: u64) -> Result<()> {
    if let Some(len) = content_length
        && len > MAX_RELEASE_ASSET_BYTES
    {
        return Err(budget_exceeded(
            "download-size",
            format!(
                "release declares {len} bytes, exceeding the {MAX_RELEASE_ASSET_BYTES}-byte download budget"
            ),
        ));
    }
    if streamed > MAX_RELEASE_ASSET_BYTES {
        return Err(budget_exceeded(
            "download-size",
            format!(
                "release streamed {streamed} bytes with no declared length, exceeding the {MAX_RELEASE_ASSET_BYTES}-byte download budget"
            ),
        ));
    }
    Ok(())
}

/// Kill `child` and its descendants (DDL-1z1). The child was spawned as its
/// own process group leader, so a negative-pid SIGKILL sweeps the whole tree.
#[cfg(unix)]
fn kill_child_tree(child: &mut std::process::Child) {
    let pid = child.id() as libc::pid_t;
    if pid > 0 {
        // SAFETY: signal dispatch only; no memory is touched.
        unsafe {
            libc::kill(-pid, libc::SIGKILL);
        }
    }
    let _ = child.kill();
    let _ = child.wait();
}

/// Kill `child` on a platform without process-group signalling (DDL-1z1).
/// Only the direct child can be killed there; descendant sweeping is a
/// POSIX capability.
#[cfg(not(unix))]
fn kill_child_tree(child: &mut std::process::Child) {
    let _ = child.kill();
    let _ = child.wait();
}

/// Run a child to completion under the DDL-1z1 runtime budget: if the child
/// (and, on POSIX, its whole process group) has not exited within `deadline`,
/// it is killed and a budget-exhaustion error naming `child-runtime` is
/// returned. Stdio is whatever the caller configured.
pub fn run_child_bounded(
    cmd: &mut std::process::Command,
    deadline: std::time::Duration,
) -> Result<std::process::ExitStatus> {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    let mut child = cmd.spawn().map_err(DdlError::Io)?;
    let start = std::time::Instant::now();
    loop {
        if let Some(status) = child.try_wait().map_err(DdlError::Io)? {
            return Ok(status);
        }
        if start.elapsed() >= deadline {
            kill_child_tree(&mut child);
            return Err(budget_exceeded(
                "child-runtime",
                format!(
                    "child '{}' exceeded the {deadline:?} runtime budget and was killed (with its process group)",
                    cmd.get_program().display()
                ),
            ));
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

/// Run a child with piped stdio under both DDL-1z1 child budgets: the
/// runtime deadline (as [`run_child_bounded`]) and the captured-output cap
/// ([`MAX_CHILD_OUTPUT_BYTES`]). Output past the cap is discarded, the child
/// tree is killed, and the budget is named in the error — no silent
/// truncation.
pub fn run_child_captured(
    cmd: &mut std::process::Command,
    deadline: std::time::Duration,
) -> Result<std::process::Output> {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    let mut child = cmd.spawn().map_err(DdlError::Io)?;

    let overflow = Arc::new(AtomicBool::new(false));
    let stdout_pipe = child.stdout.take();
    let stderr_pipe = child.stderr.take();
    let stdout_reader = drain_budgeted(stdout_pipe, Arc::clone(&overflow));
    let stderr_reader = drain_budgeted(stderr_pipe, Arc::clone(&overflow));

    let start = std::time::Instant::now();
    let mut exhausted: Option<&'static str> = None;
    let mut status = None;
    loop {
        // Budget violations win the race against a fast-exiting child: a
        // child that floods past the cap is rejected even if it exits
        // cleanly a microsecond later (no silent truncation).
        if overflow.load(Ordering::Relaxed) {
            exhausted = Some("captured-output");
            break;
        }
        if let Some(s) = child.try_wait().map_err(DdlError::Io)? {
            status = Some(s);
            break;
        }
        if stdout_reader.is_finished() && stderr_reader.is_finished() {
            // Pipes hit EOF, so the child (and any pipe-holding descendant)
            // is gone; reap its status if it is ready.
            if let Ok(Some(s)) = child.try_wait() {
                status = Some(s);
            }
            break;
        }
        if start.elapsed() >= deadline {
            exhausted = Some("child-runtime");
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    // Re-check after the loop: the child may have exited between the last
    // overflow sample and the reap.
    if exhausted.is_none() && overflow.load(Ordering::Relaxed) {
        exhausted = Some("captured-output");
    }

    if let Some(budget) = exhausted {
        kill_child_tree(&mut child);
        // The group kill closes every pipe end (descendants included), so
        // both readers reach EOF promptly — termination stays within
        // deadline + 1s.
        let _ = stdout_reader.join();
        let _ = stderr_reader.join();
        return Err(budget_exceeded(
            budget,
            format!(
                "child '{}' was killed (with its process group) for exceeding the {deadline:?} runtime budget",
                cmd.get_program().display()
            ),
        ));
    }

    let stdout = stdout_reader.join().unwrap_or_default();
    let stderr = stderr_reader.join().unwrap_or_default();
    Ok(std::process::Output {
        status: status
            .ok_or_else(|| DdlError::Other("child exited without a status".to_string()))?,
        stdout,
        stderr,
    })
}

/// Drain a child output pipe into a bounded buffer (DDL-1z1). Bytes past
/// [`MAX_CHILD_OUTPUT_BYTES`] raise `overflow` and are discarded so ddl's
/// memory stays bounded while the child is being killed.
fn drain_budgeted<R: std::io::Read + Send + 'static>(
    stream: Option<R>,
    overflow: std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut stream) = stream {
            let mut chunk = [0u8; 64 * 1024];
            loop {
                match stream.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if buf.len() + n > MAX_CHILD_OUTPUT_BYTES {
                            let keep = MAX_CHILD_OUTPUT_BYTES - buf.len();
                            buf.extend_from_slice(&chunk[..keep]);
                            overflow.store(true, std::sync::atomic::Ordering::Relaxed);
                        } else {
                            buf.extend_from_slice(&chunk[..n]);
                        }
                    }
                }
            }
        }
        buf
    })
}

/// Extract the expected binary from `bytes` into exclusive staging, then
/// atomically replace the installed executable at `dest` (DDL-4oi, SEC-01).
///
/// Guarantees:
/// - `dest` is only ever mutated by a single atomic replacement of a fully
///   extracted, executable regular file — never by a partial write.
/// - On any failure the previous bytes at `dest` are preserved exactly and
///   no staging residue remains (`.ddl-stage-*` entries are removed).
/// - Only the expected binary entry is extracted: regular files only
///   (symlinks, hardlinks, directories and special files are rejected),
///   size-bounded by [`MAX_BINARY_BYTES`], with path traversal rejected.
///
/// Archive format is detected from the bytes (gzip magic → tar.gz, otherwise
/// zip) so both the unix and Windows asset paths share one code path.
pub fn stage_and_replace_binary(
    bytes: &[u8],
    dest: &Path,
    tool_name: &str,
    url: &str,
) -> Result<()> {
    let parent = dest
        .parent()
        .ok_or_else(|| DdlError::Other("destination has no parent directory".to_string()))?;
    // Exclusive staging directory, sibling of `dest` so the final swap is a
    // same-filesystem rename.
    let staging = new_staging_dir(parent, tool_name)?;

    let result = (|| -> Result<()> {
        let staged = if bytes.starts_with(&[0x1f, 0x8b]) {
            extract_tar_gz_staged(bytes, &staging, tool_name, url)?
        } else {
            extract_zip_staged(bytes, &staging, tool_name, url)?
        };
        make_executable(&staged)?;
        atomic_replace_executable(&staged, dest)
    })();

    if result.is_err() {
        // Normal failure cases leave zero staging residue behind.
        let _ = std::fs::remove_dir_all(&staging);
    } else {
        // The staged binary was moved into place; drop the staging shell.
        let _ = std::fs::remove_dir_all(&staging);
    }
    result
}

/// Create an exclusive staging directory (create-new semantics; the attempt
/// loop disambiguates same-nanosecond collisions) named `.ddl-stage-*` so
/// cleanup passes can recognize staging residue.
fn new_staging_dir(parent: &Path, tool_name: &str) -> Result<PathBuf> {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    for attempt in 0..64u32 {
        let dir = parent.join(format!(
            ".ddl-stage-{tool_name}-{}-{}",
            std::process::id(),
            nanos + attempt as u128
        ));
        match std::fs::create_dir(&dir) {
            Ok(()) => return Ok(dir),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(DdlError::Io(e)),
        }
    }
    Err(DdlError::InstallFailed(
        "could not create an exclusive staging directory for the upgrade".to_string(),
    ))
}

/// Extract only the expected regular-file binary entry from a tar.gz into
/// `staging`. Returns the staged file path. Links, special files, oversized
/// and traversal entries are rejected before any destination mutation.
fn extract_tar_gz_staged(
    bytes: &[u8],
    staging: &Path,
    tool_name: &str,
    url: &str,
) -> Result<PathBuf> {
    let decoder = flate2::read::GzDecoder::new(bytes);
    let mut archive = tar::Archive::new(decoder);

    for entry in archive
        .entries()
        .map_err(|e| DdlError::Other(e.to_string()))?
    {
        let mut entry = entry.map_err(|e| DdlError::Other(e.to_string()))?;
        let path = entry
            .path()
            .map_err(|e| DdlError::Other(e.to_string()))?
            .to_path_buf();
        let path_display = path.display().to_string();
        if path.file_name().is_none_or(|f| f != tool_name) {
            continue;
        }
        // Only the expected regular binary is ever extracted.
        if !matches!(entry.header().entry_type(), tar::EntryType::Regular) {
            return Err(DdlError::InstallFailed(format!(
                "archive entry '{path_display}' for binary '{tool_name}' is not a regular file — rejected from {url}"
            )));
        }
        // Traversal guard: the staged copy is built manually, so the entry
        // path must stay inside the staging directory.
        if path.components().any(|c| {
            matches!(
                c,
                std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
            )
        }) {
            return Err(DdlError::InstallFailed(format!(
                "archive entry '{path_display}' attempts path traversal — rejected from {url}"
            )));
        }
        let declared = entry
            .header()
            .size()
            .map_err(|e| DdlError::Other(e.to_string()))?;
        if declared > MAX_BINARY_BYTES {
            return Err(DdlError::InstallFailed(format!(
                "archive entry '{path_display}' declares {declared} bytes, exceeding the {MAX_BINARY_BYTES}-byte binary limit — rejected from {url}"
            )));
        }
        let staged_path = staging.join(&path);
        if let Some(p) = staged_path.parent() {
            std::fs::create_dir_all(p).map_err(DdlError::Io)?;
        }
        {
            let mut out = std::fs::File::create(&staged_path).map_err(DdlError::Io)?;
            let copied = std::io::copy(&mut entry, &mut out).map_err(DdlError::Io)?;
            if copied != declared {
                return Err(DdlError::InstallFailed(format!(
                    "interrupted copy: archive entry '{path_display}' declared {declared} bytes but only {copied} could be read — {url}"
                )));
            }
            out.sync_all().map_err(DdlError::Io)?;
        }
        return Ok(staged_path);
    }

    Err(DdlError::InstallFailed(format!(
        "Binary '{tool_name}' not found in archive from {url}"
    )))
}

/// Extract only the expected regular-file binary entry from a zip into
/// `staging`. Returns the staged file path. Symlinks, directories, oversized
/// and traversal entries are rejected before any destination mutation.
fn extract_zip_staged(bytes: &[u8], staging: &Path, tool_name: &str, url: &str) -> Result<PathBuf> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes.to_vec()))
        .map_err(|e| DdlError::Other(e.to_string()))?;

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| DdlError::Other(e.to_string()))?;
        let name = entry
            .name()
            .map_err(|e| DdlError::Other(e.to_string()))?
            .into_owned();
        let base = name
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .unwrap_or(&name);
        if base != tool_name {
            continue;
        }
        // Only the expected regular binary is ever extracted.
        if entry.is_symlink() || entry.is_dir() {
            return Err(DdlError::InstallFailed(format!(
                "archive entry '{name}' for binary '{tool_name}' is not a regular file — rejected from {url}"
            )));
        }
        let declared = entry.size();
        if declared > MAX_BINARY_BYTES {
            return Err(DdlError::InstallFailed(format!(
                "archive entry '{name}' declares {declared} bytes, exceeding the {MAX_BINARY_BYTES}-byte binary limit — rejected from {url}"
            )));
        }
        // enclosed_name() rejects traversal ("../", absolute paths).
        let staged_path = staging.join(entry.enclosed_name().ok_or_else(|| {
            DdlError::InstallFailed(format!(
                "archive entry '{name}' attempts path traversal — rejected from {url}"
            ))
        })?);
        if let Some(p) = staged_path.parent() {
            std::fs::create_dir_all(p).map_err(DdlError::Io)?;
        }
        {
            let mut out = std::fs::File::create(&staged_path).map_err(DdlError::Io)?;
            let copied = std::io::copy(&mut entry, &mut out).map_err(DdlError::Io)?;
            if copied != declared {
                return Err(DdlError::InstallFailed(format!(
                    "interrupted copy: archive entry '{name}' declared {declared} bytes but only {copied} could be read — {url}"
                )));
            }
            out.sync_all().map_err(DdlError::Io)?;
        }
        return Ok(staged_path);
    }

    Err(DdlError::InstallFailed(format!(
        "Binary '{tool_name}' not found in zip archive from {url}"
    )))
}

/// Mark a staged binary executable (POSIX mode 0o755; a no-op on Windows,
/// where executability is carried by the file extension).
fn make_executable(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
            .map_err(DdlError::Io)?;
    }
    Ok(())
}

/// Atomically replace the installed executable at `dest` with the staged
/// binary (DDL-4oi).
///
/// - POSIX: `rename(2)` is atomic over an existing file — a concurrent
///   reader observes either the complete old or the complete new executable,
///   never a partial write.
/// - Windows: `std::fs::rename` fails when `dest` exists, so the replacement
///   is a defined move-aside protocol with rollback: (1) move the current
///   executable to `<dest>.ddl-old`, (2) rename the staged binary into
///   place, (3) delete the rollback file. Any failure in (2) restores the
///   old executable; concurrent upgrades are serialized by the DDL-5bs
///   install lock, closing the brief exposure window between (1) and (2).
fn atomic_replace_executable(staged: &Path, dest: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        std::fs::rename(staged, dest).map_err(DdlError::Io)?;
        Ok(())
    }
    #[cfg(not(unix))]
    {
        match std::fs::rename(staged, dest) {
            Ok(()) => Ok(()),
            Err(_) => {
                let bak = sibling_path(dest, ".ddl-old");
                let _ = std::fs::remove_file(&bak);
                std::fs::rename(dest, &bak).map_err(DdlError::Io)?;
                match std::fs::rename(staged, dest) {
                    Ok(()) => {
                        let _ = std::fs::remove_file(&bak);
                        Ok(())
                    }
                    Err(e) => {
                        // Rollback: the working executable must survive.
                        let _ = std::fs::rename(&bak, dest);
                        Err(DdlError::Io(e))
                    }
                }
            }
        }
    }
}

/// Path for a sibling of `dest` with `suffix` appended to the file name.
#[cfg(not(unix))]
fn sibling_path(dest: &Path, suffix: &str) -> PathBuf {
    let mut name = dest
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_default();
    name.push(suffix);
    match dest.parent() {
        Some(p) => p.join(name),
        None => PathBuf::from(name),
    }
}

fn install_cargo(tool: &Tool, verbose: bool) -> Result<()> {
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
fn install_npm(tool: &Tool, verbose: bool) -> Result<()> {
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

// ── Init-side repo setup (DDL-fw7) ────────────────────────────────────

/// Make `ddl init` the canonical per-repo setup entry point.
///
/// Three idempotent pieces the doctor used to only *hint* at (DDL-6zn.5):
///
/// 1. write ddl's own managed block into AGENTS.md (`agents_block()`,
///    genesis::aix provenance-aware format) — agents_hand-wiring what init
///    should wire was the dominant setup friction;
/// 2. reconcile pre-existing tool installs into `.ddl/manifest.json` —
///    doctor flagged "not tracked in manifest" with no fix;
/// 3. scaffold `versions.ddl.toml` from the fleet pin
///    ([`crate::manifest::FLEET_VERSIONS_PIN`]) when absent — doctor
///    hinted "CI installs are unpinned" but never created it.
///
/// The repo root is the enclosing git root (the `ddl.repo.*` doctor checks
/// key off the same boundary); a plain directory without git falls back to
/// the cwd, matching `DdlDir::find_or_create`. Failures are reported but
/// never abort the whole init: they are advisory setup gaps, not install
/// failures.
pub fn init_repo_setup(ddl_dir: &mut DdlDir, verbose: bool) {
    let repo_root = repo_root();

    match ensure_agents_block(&repo_root) {
        Ok(true) => eprintln!("  ✓ AGENTS.md: DDL managed block written"),
        Ok(false) => verbose_print(verbose, "AGENTS.md: DDL managed block already present"),
        Err(e) => eprintln!("  ⚠ could not write DDL managed block to AGENTS.md: {e}"),
    }

    match scaffold_versions_pin(&repo_root) {
        Ok(true) => eprintln!("  ✓ versions.ddl.toml: scaffolded from the fleet pin"),
        Ok(false) => verbose_print(verbose, "versions.ddl.toml already present"),
        Err(e) => eprintln!("  ⚠ could not scaffold versions.ddl.toml: {e}"),
    }

    match reconcile_existing_installs(ddl_dir) {
        Ok(reconciled) if reconciled.is_empty() => {
            verbose_print(verbose, "manifest: no pre-existing installs to reconcile")
        }
        Ok(reconciled) => eprintln!(
            "  ✓ manifest: reconciled pre-existing installs ({})",
            reconciled.join(", ")
        ),
        Err(e) => eprintln!("  ⚠ could not reconcile manifest: {e}"),
    }
}

/// The repo root for init's repo-scoped setup files: the enclosing git
/// root, or the cwd when there is none (same fallback semantics as
/// `DdlDir::find_or_create`).
fn repo_root() -> PathBuf {
    crate::diagnostics::find_git_root(Path::new(".")).unwrap_or_else(|| PathBuf::from("."))
}

/// Write ddl's own managed block into AGENTS.md at `repo_root` if absent.
/// Returns `Ok(false)` when the block is already present (idempotent);
/// creates the file when missing. Returns `Ok(true)` when written.
pub fn ensure_agents_block(repo_root: &Path) -> Result<bool> {
    let path = repo_root.join("AGENTS.md");
    let existing = path
        .exists()
        .then(|| std::fs::read_to_string(&path))
        .transpose()
        .map_err(DdlError::Io)?;
    if existing
        .as_deref()
        .is_some_and(|text| text.contains(crate::DDL_AGENTS_MARKER))
    {
        return Ok(false);
    }
    let block = crate::agents_block();
    let mut text = existing.unwrap_or_default();
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str(&block);
    text.push('\n');
    std::fs::write(&path, text).map_err(DdlError::Io)?;
    Ok(true)
}

/// Scaffold `versions.ddl.toml` at `repo_root` from the fleet pin when the
/// repo has none. Returns `Ok(false)` when a pins file already exists —
/// a repo's committed pins are its own source of truth and are never
/// overwritten by init.
pub fn scaffold_versions_pin(repo_root: &Path) -> Result<bool> {
    let path = repo_root.join("versions.ddl.toml");
    if path.exists() {
        return Ok(false);
    }
    std::fs::write(&path, crate::manifest::FLEET_VERSIONS_PIN).map_err(DdlError::Io)?;
    Ok(true)
}

/// Reconcile pre-existing tool installs into the per-repo manifest.
///
/// For every managed tool that is installed (on PATH) but not tracked in
/// `.ddl/manifest.json`, record an `installed` entry with the probed
/// version and a reconciled source. Returns the reconciled tool names.
/// This is inventory bookkeeping only — it never touches the install or
/// verification protocol (DDL-yts/5bs/uu8 territory).
pub fn reconcile_existing_installs(ddl_dir: &mut DdlDir) -> Result<Vec<String>> {
    let mut reconciled = Vec::new();
    for tool in MANAGED_TOOLS {
        if !is_tool_installed(tool.name) || ddl_dir.manifest.is_installed(tool.name) {
            continue;
        }
        let version = get_installed_version(tool.name).unwrap_or_else(|| "unknown".to_string());
        ddl_dir.record_installed(
            tool.name,
            &version,
            "pre-existing install (reconciled by ddl init)",
        )?;
        reconciled.push(tool.name.to_string());
    }
    Ok(reconciled)
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
    // DDL-uu8: restricted child environment; prepend the ddl bin dir so
    // downloaded tools see their siblings (override wins over the allowlist).
    let mut overrides: Vec<(&str, std::ffi::OsString)> = Vec::new();
    if let Some(bin_dir) = ddl_bin_dir() {
        overrides.push((
            "PATH",
            prepend_path(&bin_dir, std::env::var_os("PATH").as_deref()),
        ));
    }
    apply_child_policy(&mut command, ExecutionScope::ToolInit, &overrides).map_err(|e| {
        DdlError::InstallFailed(format!("{} init rejected by child policy: {e}", tool.name))
    })?;

    // DDL-1z1: the init child runs under the runtime and captured-output
    // budgets — a hung or flooding init child is killed (with its process
    // group) and the budget is named in the failure, which feeds the resume
    // summary and JSON diagnostics.
    let output = run_child_captured(&mut command, init_deadline_for(tool.name))?;

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
fn upgrade_cargo(tool: &Tool, verbose: bool) -> Result<()> {
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
fn upgrade_npm(tool: &Tool, verbose: bool) -> Result<()> {
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
fn upgrade_binary(tool: &Tool, platform: &Platform, verbose: bool) -> Result<VerificationStatus> {
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
pub const ALLOW_GLOBAL_SKILL_ENV: &str = "DDL_ALLOW_GLOBAL_SKILL";

/// Legacy-transition variable: when set to `1`, children inherit the caller's
/// full environment instead of the restricted allowlist. Mirrors the
/// `DDL_ALLOW_UNVERIFIED_RELEASE` explicit-authorization semantics.
pub const ALLOW_UNRESTRICTED_CHILD_ENV: &str = "DDL_ALLOW_UNRESTRICTED_CHILD_ENV";

/// The execution scope of a child spawned by ddl.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionScope {
    /// Package build/install (`cargo install`, `npm install -g`) — authorized
    /// by the explicit user-invoked install/upgrade command itself.
    PackageBuild,
    /// Global skill installation — writes outside this project and affects
    /// other projects; requires explicit authorization via
    /// [`ALLOW_GLOBAL_SKILL_ENV`].
    GlobalSkill,
    /// Per-tool init cascade — writes are confined to the invoking project
    /// tree and ddl's own bin dir; authorized by the explicit `ddl init`.
    ToolInit,
}

impl ExecutionScope {
    pub fn label(self) -> &'static str {
        match self {
            Self::PackageBuild => "package-build",
            Self::GlobalSkill => "global-skill",
            Self::ToolInit => "tool-init",
        }
    }

    /// The env var granting explicit authorization for this scope, if any.
    /// Scopes writing only inside this project (or explicitly requested by
    /// the user's own install/upgrade command) need no additional grant.
    pub fn authorization_env(self) -> Option<&'static str> {
        match self {
            Self::GlobalSkill => Some(ALLOW_GLOBAL_SKILL_ENV),
            Self::PackageBuild | Self::ToolInit => None,
        }
    }
}

/// Platform support matrix: the restricted-privilege mechanism (controlled
/// env dictionary + explicit scope authorization) is portable across every
/// platform std supports. Any other target must be rejected explicitly —
/// never silently run children unsandboxed.
pub fn child_policy_supported() -> bool {
    cfg!(any(
        target_os = "linux",
        target_os = "macos",
        target_os = "windows"
    ))
}

/// Environment allowlist for children spawned by ddl. Everything NOT listed
/// here (credentials, tokens, secrets) is dropped from the child environment.
#[cfg(not(windows))]
pub const CHILD_ENV_ALLOWLIST: &[&str] = &[
    "PATH",
    "HOME",
    "USER",
    "LOGNAME",
    "SHELL",
    "TERM",
    "TMPDIR",
    "LANG",
    "LC_ALL",
    "XDG_CONFIG_HOME",
    "XDG_DATA_HOME",
    "XDG_CACHE_HOME",
    "XDG_RUNTIME_DIR",
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "NO_PROXY",
    "http_proxy",
    "https_proxy",
    "no_proxy",
    "SSL_CERT_FILE",
    "SSL_CERT_DIR",
    "CARGO_HOME",
    "RUSTUP_HOME",
    "NPM_CONFIG_PREFIX",
];

/// Environment allowlist for children spawned by ddl (Windows variants:
/// cmd shim routing and npm/cargo need the system profile variables).
#[cfg(windows)]
pub const CHILD_ENV_ALLOWLIST: &[&str] = &[
    "PATH",
    "PATHEXT",
    "COMSPEC",
    "SYSTEMROOT",
    "WINDIR",
    "SystemDrive",
    "USERPROFILE",
    "HOMEDRIVE",
    "HOMEPATH",
    "APPDATA",
    "LOCALAPPDATA",
    "PROGRAMDATA",
    "ALLUSERSPROFILE",
    "PROGRAMFILES",
    "COMMONPROGRAMFILES",
    "TEMP",
    "TMP",
    "USERNAME",
    "USERDOMAIN",
    "COMPUTERNAME",
    "LANG",
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "NO_PROXY",
    "SSL_CERT_FILE",
    "SSL_CERT_DIR",
    "CARGO_HOME",
    "RUSTUP_HOME",
    "NPM_CONFIG_PREFIX",
];

/// Is the legacy unrestricted-child-environment transition explicitly
/// authorized?
fn unrestricted_child_env_authorized() -> bool {
    std::env::var(ALLOW_UNRESTRICTED_CHILD_ENV).as_deref() == Ok("1")
}

/// Build the child environment from the policy allowlist, applying explicit
/// overrides last (so e.g. the tool-init PATH prepend always wins).
///
/// Fail-closed: without a usable PATH the child cannot be started sanely, so
/// the policy refuses instead of inheriting a half-broken environment.
pub fn sanitize_child_env(
    overrides: &[(&str, std::ffi::OsString)],
) -> Result<Vec<(String, std::ffi::OsString)>> {
    let mut env: Vec<(String, std::ffi::OsString)> = Vec::new();
    if unrestricted_child_env_authorized() {
        for (k, v) in std::env::vars_os() {
            env.push((k.to_string_lossy().into_owned(), v));
        }
    } else {
        for key in CHILD_ENV_ALLOWLIST {
            if let Some(v) = std::env::var_os(key) {
                env.push(((*key).to_string(), v));
            }
        }
    }
    for (k, v) in overrides {
        env.retain(|(existing, _)| existing.as_str() != *k);
        env.push(((*k).to_string(), v.clone()));
    }
    if !env.iter().any(|(k, _)| k == "PATH") {
        return Err(DdlError::InstallFailed(
            "child environment policy (DDL-uu8): PATH is missing — refusing to spawn a child without an executable search path".to_string(),
        ));
    }
    Ok(env)
}

/// Apply the restricted-privilege policy to a child command: replace the
/// inherited environment with the allowlisted one and reject execution on
/// unsupported platforms. Zero silent unsandboxed fallback.
pub fn apply_child_policy(
    command: &mut Command,
    scope: ExecutionScope,
    overrides: &[(&str, std::ffi::OsString)],
) -> Result<()> {
    if !child_policy_supported() {
        return Err(DdlError::UnsupportedPlatform(format!(
            "restricted child execution policy (DDL-uu8) has no enforcement mechanism on this platform — refusing to run the {} child unsandboxed",
            scope.label()
        )));
    }
    let env = sanitize_child_env(overrides)?;
    command.env_clear();
    for (k, v) in env {
        command.env(k, v);
    }
    Ok(())
}

/// Approve (or reject) a child execution scope. Scopes with an authorization
/// env var are rejected unless it is explicitly set to `1`; the rejection
/// message names the variable so the remedy is discoverable.
pub fn authorize_scope(scope: ExecutionScope) -> Result<()> {
    scope_authorize_with(scope, |var| std::env::var(var).ok())
}

/// Pure decision core of [`authorize_scope`], injectable for tests.
pub fn scope_authorize_with<F>(scope: ExecutionScope, lookup: F) -> Result<()>
where
    F: Fn(&str) -> Option<String>,
{
    match scope.authorization_env() {
        // Authorized by the explicit user-invoked install/upgrade/init command.
        None => Ok(()),
        Some(var) if lookup(var).as_deref() == Some("1") => Ok(()),
        Some(var) => Err(DdlError::InstallFailed(format!(
            "{} scope rejected by the restricted-privilege policy (DDL-uu8): its writes reach beyond this project and affect other projects. Re-run with {var}=1 to explicitly authorize, or install skills locally instead.",
            scope.label()
        ))),
    }
}

/// The approved write roots for a child execution scope. Anything outside
/// these roots is outside the child's declared filesystem authority.
pub fn child_write_roots(scope: ExecutionScope, project_root: Option<&Path>) -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();
    if let Some(project) = project_root {
        roots.push(project.to_path_buf());
    }
    if let Some(bin) = ddl_bin_dir() {
        roots.push(bin);
    }
    match scope {
        // Global package registries/caches (cargo home) are the package-build
        // scope's declared target.
        ExecutionScope::PackageBuild => {
            if let Some(home) = std::env::var_os("CARGO_HOME") {
                roots.push(PathBuf::from(home));
            }
        }
        // Global skills live under ~/.agents/skills — the global-skill
        // scope's declared target.
        ExecutionScope::GlobalSkill => {
            if let Some(dir) = crate::skills::global_skills_dir() {
                roots.push(dir);
            }
        }
        ExecutionScope::ToolInit => {}
    }
    roots
}

/// Reject a path outside the approved write roots of `scope`.
pub fn check_write_root(
    scope: ExecutionScope,
    path: &Path,
    project_root: Option<&Path>,
) -> Result<()> {
    let roots = child_write_roots(scope, project_root);
    if roots.iter().any(|root| path.starts_with(root)) {
        Ok(())
    } else {
        let listed = roots
            .iter()
            .map(|r| r.display().to_string())
            .collect::<Vec<_>>()
            .join(", ");
        Err(DdlError::InstallFailed(format!(
            "write to {} is outside the approved roots of the {} scope (DDL-uu8): approved roots: {listed}",
            path.display(),
            scope.label()
        )))
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

    // ===== DDL-uu8 (SEC-04): restricted-privilege policy =====

    #[test]
    fn scope_authorization_matrix() {
        // GlobalSkill requires explicit DDL_ALLOW_GLOBAL_SKILL=1.
        assert!(scope_authorize_with(ExecutionScope::GlobalSkill, |_| None).is_err());
        assert!(
            scope_authorize_with(ExecutionScope::GlobalSkill, |v| {
                if v == ALLOW_GLOBAL_SKILL_ENV {
                    Some("1".into())
                } else {
                    None
                }
            })
            .is_ok()
        );
        // PackageBuild and ToolInit are authorized by the explicit
        // user-invoked command itself (no extra grant).
        assert!(scope_authorize_with(ExecutionScope::PackageBuild, |_| None).is_ok());
        assert!(scope_authorize_with(ExecutionScope::ToolInit, |_| None).is_ok());
    }

    #[test]
    fn scope_rejection_message_names_the_authorization_variable() {
        let err = scope_authorize_with(ExecutionScope::GlobalSkill, |_| None).unwrap_err();
        assert!(err.to_string().contains(ALLOW_GLOBAL_SKILL_ENV));
    }

    #[test]
    fn write_roots_reject_out_of_scope_paths() {
        let temp = tempfile::tempdir().unwrap();
        let project = temp.path().join("project");
        std::fs::create_dir_all(&project).unwrap();
        // Inside the project root: approved for tool-init.
        assert!(
            check_write_root(
                ExecutionScope::ToolInit,
                &project.join("openspec"),
                Some(&project)
            )
            .is_ok()
        );
        // Outside every approved root: rejected.
        assert!(
            check_write_root(
                ExecutionScope::ToolInit,
                Path::new("/etc/passwd"),
                Some(&project)
            )
            .is_err()
        );
        // The global-skill scope's declared target is ~/.agents/skills.
        if let Some(skills_dir) = crate::skills::global_skills_dir() {
            assert!(
                check_write_root(
                    ExecutionScope::GlobalSkill,
                    &skills_dir.join("commit"),
                    Some(&project)
                )
                .is_ok()
            );
            assert!(
                check_write_root(
                    ExecutionScope::GlobalSkill,
                    Path::new("/etc/passwd"),
                    Some(&project)
                )
                .is_err()
            );
        }
    }

    #[test]
    fn child_env_allowlist_excludes_credentials() {
        assert!(CHILD_ENV_ALLOWLIST.contains(&"PATH"));
        assert!(CHILD_ENV_ALLOWLIST.contains(&"HOME"));
        assert!(!CHILD_ENV_ALLOWLIST.contains(&"DDL_TEST_CANARY_CREDENTIAL"));
        assert!(!CHILD_ENV_ALLOWLIST.contains(&"GITHUB_TOKEN"));
    }

    #[test]
    fn child_policy_platform_matrix_is_supported() {
        // The env-allowlist + authorization mechanism is portable across the
        // platforms ddl supports; anything else must fail closed.
        assert!(child_policy_supported());
    }

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
    fn test_release_tag_from_url() {
        assert_eq!(
            release_tag_from_url(
                "https://github.com/charly-vibes/dulce-de-leche/releases/download/v0.8.0/ddl_0.8.0_linux_amd64.tar.gz"
            ),
            Some("v0.8.0")
        );
        assert_eq!(
            release_tag_from_url("https://example.com/nightly/tool.tar.gz"),
            None
        );
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
