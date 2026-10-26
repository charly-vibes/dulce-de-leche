//! Contract bundle verification and base caching (wild-xgd, rung 2).
//!
//! A contract bundle is the release-CI-attached sidecar carrying a tool's
//! extracted wild contract (`contract.json`) plus its digest-bound trust-chain
//! head (`manifest.toml`). Spec source: openspec change
//! `add-wild-contract-bundle-convention` (wild repo) — the wild-registry
//! delta's bundle refusal laws and base-contract caching requirements.
//!
//! Refusal law (never silent acceptance): a bundle whose recomputed
//! sha256 of `contract.json` differs from `manifest.contract_sha256` is
//! refused (`bundle-digest-mismatch`); a bundle whose `manifest.commit`
//! does not match the release being upgraded to is refused
//! (`bundle-commit-mismatch`); anything malformed is refused
//! (`bundle-malformed`). The cached installed base is digest-verified
//! against its recorded digest on every use; a missing cache yields
//! `missing-base-contract` and a corrupt one `cached-base-digest-mismatch`
//! — a fabricated or silently skipped base contract is never used.

use crate::error::{DdlError, Result};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::path::Path;

/// The parsed `manifest.toml` — the digest-bound trust-chain head. Anything
/// absent from it is unverifiable by construction.
#[derive(Debug, Clone, Deserialize)]
pub struct BundleManifest {
    pub tool: String,
    pub version: String,
    pub commit: String,
    pub profile: String,
    pub contract_sha256: String,
    pub extractor_version: String,
    pub extracted_at: String,
}

/// A bundle that passed the refusal law: manifest parsed, digest recomputed
/// and matched, commit matched against the expected release commit.
#[derive(Debug, Clone)]
pub struct VerifiedBundle {
    pub manifest: BundleManifest,
    /// The exact `contract.json` bytes (digest-verified).
    pub contract_json: Vec<u8>,
    /// The recomputed sha256 hex of `contract.json`.
    pub contract_sha256: String,
}

/// A bundle refusal naming its diagnostic. Every refusal carries the named
/// diagnostic code as a prefix so rendering and tests can never lose which
/// check failed.
#[derive(Debug, thiserror::Error)]
#[error("{code}: {message}")]
pub struct BundleError {
    code: &'static str,
    message: String,
}

impl BundleError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    /// The named diagnostic code (`bundle-digest-mismatch`,
    /// `bundle-commit-mismatch`, `bundle-malformed`, `missing-base-contract`,
    /// `cached-base-digest-mismatch`).
    pub fn code(&self) -> &'static str {
        self.code
    }
}

impl From<BundleError> for DdlError {
    fn from(e: BundleError) -> Self {
        DdlError::Other(e.to_string())
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

/// Recompute the sha256 of `contract.json` against `manifest.contract_sha256`,
/// then check `manifest.commit` against the expected release commit. Every
/// refusal names its diagnostic and nothing is silently accepted.
pub fn verify_bundle(
    dir: &Path,
    expected_commit: &str,
) -> std::result::Result<VerifiedBundle, BundleError> {
    // The bundle is exactly two manifest-bound files; anything else in the
    // directory is not the convention and is refused, not tolerated.
    let entries = std::fs::read_dir(dir)
        .map_err(|e| BundleError::new("bundle-malformed", format!("bundle dir unreadable: {e}")))?;
    let mut names: Vec<String> = Vec::new();
    for entry in entries {
        let name = entry
            .map_err(|e| {
                BundleError::new("bundle-malformed", format!("bundle dir unreadable: {e}"))
            })?
            .file_name()
            .to_string_lossy()
            .to_string();
        names.push(name);
    }
    names.sort();
    if names != ["contract.json", "manifest.toml"] {
        return Err(BundleError::new(
            "bundle-malformed",
            format!("bundle must contain exactly contract.json and manifest.toml, found {names:?}"),
        ));
    }

    let manifest_text = std::fs::read_to_string(dir.join("manifest.toml")).map_err(|e| {
        BundleError::new("bundle-malformed", format!("manifest.toml unreadable: {e}"))
    })?;
    let manifest: BundleManifest = toml::from_str(&manifest_text).map_err(|e| {
        BundleError::new("bundle-malformed", format!("manifest.toml malformed: {e}"))
    })?;

    let contract_json = std::fs::read(dir.join("contract.json")).map_err(|e| {
        BundleError::new("bundle-malformed", format!("contract.json unreadable: {e}"))
    })?;

    // Refusal law, first clause: recomputed digest vs stamped digest.
    let recomputed = sha256_hex(&contract_json);
    if recomputed != manifest.contract_sha256 {
        return Err(BundleError::new(
            "bundle-digest-mismatch",
            format!(
                "recomputed sha256(contract.json) = {recomputed} but manifest stamps {}",
                manifest.contract_sha256
            ),
        ));
    }

    // Refusal law, second clause: stamped commit vs the release being
    // upgraded to.
    if manifest.commit != expected_commit {
        return Err(BundleError::new(
            "bundle-commit-mismatch",
            format!(
                "manifest.commit = {} but the target release commit is {expected_commit}",
                manifest.commit
            ),
        ));
    }

    Ok(VerifiedBundle {
        contract_sha256: recomputed,
        manifest,
        contract_json,
    })
}

/// Cache a verified bundle as the installed base under the ddl state dir:
/// `contracts/<tool>/<version>/`. The cached copy is the verified bytes,
/// so the digest recorded by the caller stays honest.
pub fn cache_base_bundle(
    state_dir: &Path,
    tool: &str,
    version: &str,
    verified: &VerifiedBundle,
) -> Result<()> {
    let dest = state_dir.join("contracts").join(tool).join(version);
    std::fs::create_dir_all(&dest)?;
    std::fs::write(dest.join("contract.json"), &verified.contract_json)?;
    // Re-serialize the manifest verbatim fields — the cached manifest is the
    // same trust-chain head, regenerated from the verified record.
    let m = &verified.manifest;
    let manifest_text = format!(
        "tool = '{}'\nversion = '{}'\ncommit = '{}'\nprofile = '{}'\ncontract_sha256 = '{}'\nextractor_version = '{}'\nextracted_at = '{}'\n",
        m.tool,
        m.version,
        m.commit,
        m.profile,
        m.contract_sha256,
        m.extractor_version,
        m.extracted_at
    );
    std::fs::write(dest.join("manifest.toml"), manifest_text)?;
    Ok(())
}

/// Load and digest-verify the cached base bundle for an installed tool.
/// A missing cache yields `missing-base-contract`; a corrupt cache yields
/// `cached-base-digest-mismatch`. Neither is fabricated or silently skipped.
pub fn load_cached_base(
    state_dir: &Path,
    tool: &str,
    version: &str,
    recorded_contract_sha256: &str,
) -> std::result::Result<VerifiedBundle, BundleError> {
    let dir = state_dir.join("contracts").join(tool).join(version);
    if !dir.is_dir() {
        return Err(BundleError::new(
            "missing-base-contract",
            format!(
                "no cached contract bundle for {tool} {version} under {}",
                state_dir.display()
            ),
        ));
    }
    // The cached base verifies against the lock entry's recorded digest —
    // the commitment settled when the base was cached — so the digest clause
    // runs directly against the recorded value rather than the stamped one.
    let manifest_text = std::fs::read_to_string(dir.join("manifest.toml")).map_err(|e| {
        BundleError::new(
            "bundle-malformed",
            format!("cached manifest.toml unreadable: {e}"),
        )
    })?;
    let manifest: BundleManifest = toml::from_str(&manifest_text).map_err(|e| {
        BundleError::new(
            "bundle-malformed",
            format!("cached manifest.toml malformed: {e}"),
        )
    })?;
    let contract_json = std::fs::read(dir.join("contract.json")).map_err(|e| {
        BundleError::new(
            "bundle-malformed",
            format!("cached contract.json unreadable: {e}"),
        )
    })?;
    let recomputed = sha256_hex(&contract_json);
    if recomputed != recorded_contract_sha256 {
        return Err(BundleError::new(
            "cached-base-digest-mismatch",
            format!(
                "cached base for {tool} {version} recomputes to {recomputed} but the lock entry records {recorded_contract_sha256}"
            ),
        ));
    }
    Ok(VerifiedBundle {
        manifest,
        contract_json,
        contract_sha256: recomputed,
    })
}
