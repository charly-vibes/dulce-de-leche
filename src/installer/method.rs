//! Install method selection (DDL-ei3 policy types and decision helpers).

use super::*;
use crate::error::{DdlError, Result};
use crate::manifest::{InstallLock, LockEntry};
use crate::platform::{Platform, Tool};

/// Outcome of release-artifact verification for an installed binary.
/// `label()` is the machine-readable value used in JSON output events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerificationStatus {
    /// sha256 matched the checksum published for the release, AND the
    /// provenance manifest bound those bytes to the expected build identity
    /// (charly-vibes origins).
    Verified,
    /// sha256 matched the release's own checksums.txt / per-asset digest but
    /// no provenance manifest was required: external origins (non-
    /// charly-vibes) can never publish our provenance.json, so they verify
    /// through the weaker checksums-only channel (DDL-9x4.11.1).
    ChecksumsVerified,
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
            Self::ChecksumsVerified => "checksums-verified",
            Self::UnverifiedAuthorized => "unverified-authorized",
            Self::Unverified => "unverified",
        }
    }

    /// Human-readable suffix appended to install result messages so human and
    /// JSON output both carry the distinction.
    pub fn suffix(self) -> String {
        match self {
            Self::Verified => " (sha256 verified)".to_string(),
            Self::ChecksumsVerified => {
                " (sha256 verified against the release's own checksums — external origin)"
                    .to_string()
            }
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
    /// `to` is boxed: LockEntry grew past the clippy large-enum-variant
    /// threshold when it gained the contract digest (wild-xgd).
    Upgrade { from: LockEntry, to: Box<LockEntry> },
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
        to: Box::new(LockEntry {
            tool: current.tool.clone(),
            version: candidate.version.clone(),
            artifact_url: candidate.artifact_url.clone(),
            sha256: candidate.sha256.clone(),
            recorded_epoch_day: today_epoch_day,
            // Unknown until the candidate's bundle is fetched and verified
            // during the upgrade — the shadow path records it then (wild-xgd).
            contract_sha256: None,
        }),
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
