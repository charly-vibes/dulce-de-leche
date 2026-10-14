//! Integration tests for the DDL-ei3 install policy: binary-first on every
//! platform, cargo as the runtime fallback, npm only for incitaciones.
//! No network access — method selection and fallback decisions are pure.

use dulce_de_leche::installer::{
    InstallMethod, best_install_method, fallback_after_binary_failure,
};
use dulce_de_leche::platform::{Arch, MANAGED_TOOLS, Os, Platform};

fn all_platforms() -> Vec<Platform> {
    use Arch::{Amd64, Arm64};
    use Os::{Linux, Macos, Windows};
    vec![
        Platform {
            os: Macos,
            arch: Arm64,
        },
        Platform {
            os: Macos,
            arch: Amd64,
        },
        Platform {
            os: Linux,
            arch: Arm64,
        },
        Platform {
            os: Linux,
            arch: Amd64,
        },
        Platform {
            os: Windows,
            arch: Amd64,
        },
    ]
}

#[test]
fn method_matrix_is_binary_first_with_npm_exception() {
    for platform in all_platforms() {
        for tool in MANAGED_TOOLS {
            let method = best_install_method(tool, &platform);
            if tool.npm_package.is_some() {
                assert_eq!(
                    method,
                    InstallMethod::Npm,
                    "npm tool `{}` must install via npm on {}",
                    tool.name,
                    platform.os
                );
            } else {
                assert_eq!(
                    method,
                    InstallMethod::Binary,
                    "tool `{}` must be binary-first on {}",
                    tool.name,
                    platform.os
                );
            }
        }
    }
}

#[test]
fn fallback_only_when_cargo_available() {
    assert_eq!(
        fallback_after_binary_failure(true),
        Some(InstallMethod::Cargo)
    );
    assert_eq!(fallback_after_binary_failure(false), None);
}

// ---------------------------------------------------------------------------
// DDL-wof: reject release artifacts from an unexpected publisher or build identity.
//
// Pure-policy fixtures (no network): `enforce_provenance_policy` receives the
// provenance lookup result (published manifest / unavailable) and the expected
// release tag parsed from the archive URL, and decides verified / rejected /
// explicitly-authorized. Expected trust roots are constants in installer.rs
// (repository + workflow name of ddl's own release workflow); the manifest's
// per-asset digest binds identity to the downloaded bytes.
// ---------------------------------------------------------------------------

use dulce_de_leche::installer::{
    EXPECTED_PROVENANCE_REPOSITORY, EXPECTED_PROVENANCE_WORKFLOW, ProvenanceLookup,
    ProvenanceManifest, enforce_provenance_policy,
};

/// The asset filename carved out of FIXTURE_URL's last path segment.
const FIXTURE_ASSET: &str = "tool_v1.0.0_linux_amd64.tar.gz";

fn manifest(repository: &str, workflow: &str, tag: &str, digest: &str) -> ProvenanceManifest {
    ProvenanceManifest {
        repository: repository.to_string(),
        workflow: workflow.to_string(),
        tag: tag.to_string(),
        artifacts: [(FIXTURE_ASSET.to_string(), digest.to_string())].into(),
    }
}

fn provenance_policy(
    lookup: ProvenanceLookup,
    expected_tag: Option<&str>,
    allow_unverified: bool,
) -> Result<VerificationStatus, DdlError> {
    enforce_provenance_policy(
        "ddl",
        FIXTURE_URL,
        expected_tag,
        lookup,
        FIXTURE_BYTES,
        allow_unverified,
    )
}

#[test]
fn provenance_with_expected_identity_and_matching_digest_is_accepted() {
    let status = provenance_policy(
        ProvenanceLookup::Published(manifest(
            EXPECTED_PROVENANCE_REPOSITORY,
            EXPECTED_PROVENANCE_WORKFLOW,
            "v1.0.0",
            FIXTURE_DIGEST,
        )),
        Some("v1.0.0"),
        false,
    )
    .unwrap();
    assert_eq!(status, VerificationStatus::Verified);
}

#[test]
fn altered_bytes_are_rejected_against_the_provenance_digest() {
    // Provenance binds the artifact digest to the publisher identity; bytes
    // differing from the provenance-listed digest are rejected even though the
    // identity fields themselves are correct.
    let other = "d121be3103007b41edf96f8262925f8c7d61894afe9a041843b631f69445bc57";
    let err = provenance_policy(
        ProvenanceLookup::Published(manifest(
            EXPECTED_PROVENANCE_REPOSITORY,
            EXPECTED_PROVENANCE_WORKFLOW,
            "v1.0.0",
            other,
        )),
        Some("v1.0.0"),
        false,
    )
    .unwrap_err();
    assert!(
        matches!(err, DdlError::ChecksumMismatch { .. }),
        "altered bytes must surface as ChecksumMismatch, got: {err}"
    );
}

#[test]
fn wrong_repository_identity_is_rejected() {
    let err = provenance_policy(
        ProvenanceLookup::Published(manifest(
            "evil/fork",
            EXPECTED_PROVENANCE_WORKFLOW,
            "v1.0.0",
            FIXTURE_DIGEST,
        )),
        Some("v1.0.0"),
        false,
    )
    .unwrap_err();
    assert!(
        matches!(err, DdlError::ChecksumUnavailable { .. }),
        "wrong repository must reject, got: {err}"
    );
    assert!(
        err.to_string().contains("repository"),
        "diagnostic must name the repository mismatch: {err}"
    );
}

#[test]
fn wrong_workflow_identity_is_rejected() {
    let err = provenance_policy(
        ProvenanceLookup::Published(manifest(
            EXPECTED_PROVENANCE_REPOSITORY,
            "Nightly",
            "v1.0.0",
            FIXTURE_DIGEST,
        )),
        Some("v1.0.0"),
        false,
    )
    .unwrap_err();
    assert!(matches!(err, DdlError::ChecksumUnavailable { .. }));
    assert!(
        err.to_string().contains("workflow"),
        "diagnostic must name the workflow mismatch: {err}"
    );
}

#[test]
fn wrong_release_tag_is_rejected() {
    // The manifest describes a *different* release's build than the tag the
    // asset was fetched from — identity does not match the build in use.
    let err = provenance_policy(
        ProvenanceLookup::Published(manifest(
            EXPECTED_PROVENANCE_REPOSITORY,
            EXPECTED_PROVENANCE_WORKFLOW,
            "v0.9.0",
            FIXTURE_DIGEST,
        )),
        Some("v1.0.0"),
        false,
    )
    .unwrap_err();
    assert!(matches!(err, DdlError::ChecksumUnavailable { .. }));
    assert!(
        err.to_string().contains("tag"),
        "diagnostic must name the tag mismatch: {err}"
    );
}

#[test]
fn missing_provenance_is_rejected_by_default() {
    let err = provenance_policy(
        ProvenanceLookup::Unavailable("release publishes no provenance manifest"),
        Some("v1.0.0"),
        false,
    )
    .unwrap_err();
    assert!(matches!(err, DdlError::ChecksumUnavailable { .. }));
}

#[test]
fn authorization_permits_missing_provenance() {
    let status = provenance_policy(
        ProvenanceLookup::Unavailable("release publishes no provenance manifest"),
        Some("v1.0.0"),
        true,
    )
    .unwrap();
    assert_eq!(status, VerificationStatus::UnverifiedAuthorized);
}

#[test]
fn authorization_never_rescues_wrong_identity() {
    let err = provenance_policy(
        ProvenanceLookup::Published(manifest(
            "evil/fork",
            EXPECTED_PROVENANCE_WORKFLOW,
            "v1.0.0",
            FIXTURE_DIGEST,
        )),
        Some("v1.0.0"),
        true,
    )
    .unwrap_err();
    assert!(matches!(err, DdlError::ChecksumUnavailable { .. }));
}

#[test]
fn authorization_never_rescues_altered_bytes() {
    let other = "d121be3103007b41edf96f8262925f8c7d61894afe9a041843b631f69445bc57";
    let err = provenance_policy(
        ProvenanceLookup::Published(manifest(
            EXPECTED_PROVENANCE_REPOSITORY,
            EXPECTED_PROVENANCE_WORKFLOW,
            "v1.0.0",
            other,
        )),
        Some("v1.0.0"),
        true,
    )
    .unwrap_err();
    assert!(matches!(err, DdlError::ChecksumMismatch { .. }));
}

#[test]
fn untagged_archive_url_cannot_be_provenance_checked() {
    // Without a tag parsed from the URL there is no build identity to check
    // the manifest against — rejected by default like any other unverifiable
    // download.
    let err = enforce_provenance_policy(
        "ddl",
        "https://example.com/nightly/tool.tar.gz",
        None,
        ProvenanceLookup::Published(manifest(
            EXPECTED_PROVENANCE_REPOSITORY,
            EXPECTED_PROVENANCE_WORKFLOW,
            "v1.0.0",
            FIXTURE_DIGEST,
        )),
        FIXTURE_BYTES,
        false,
    )
    .unwrap_err();
    assert!(matches!(err, DdlError::ChecksumUnavailable { .. }));
}

// ---------------------------------------------------------------------------
// DDL-yts: reject release downloads when required verification is unavailable
//
// Pure-policy fixtures (no network): `enforce_checksum_policy` receives the
// checksum lookup result (missing / malformed / not-listed / transport-failed
// / published) and decides verified / rejected / explicitly-authorized. The
// policy performs no filesystem mutations — rejection happens strictly before
// extraction touches the destination (see installer.rs call ordering).
// ---------------------------------------------------------------------------

use dulce_de_leche::error::DdlError;
use dulce_de_leche::installer::{ChecksumLookup, VerificationStatus, enforce_checksum_policy};

const FIXTURE_BYTES: &[u8] = b"ddl-fixture-binary";
/// sha256(b"ddl-fixture-binary")
const FIXTURE_DIGEST: &str = "94cc63c7a3772ded6ffe283fbfd15557699b968a81327cb5b7c5ea0ab3b202fc";
const FIXTURE_URL: &str =
    "https://github.com/acme/tool/releases/download/v1.0.0/tool_v1.0.0_linux_amd64.tar.gz";

fn policy(lookup: ChecksumLookup, allow_unverified: bool) -> Result<VerificationStatus, DdlError> {
    enforce_checksum_policy("tool", FIXTURE_URL, lookup, FIXTURE_BYTES, allow_unverified)
}

#[test]
fn missing_checksum_is_rejected_by_default() {
    let err = policy(
        ChecksumLookup::Unavailable(
            "release publishes no checksum for tool_v1.0.0_linux_amd64.tar.gz",
        ),
        false,
    )
    .unwrap_err();
    assert!(
        matches!(err, DdlError::ChecksumUnavailable { .. }),
        "missing checksum must reject, got: {err}"
    );
}

#[test]
fn malformed_checksums_txt_is_rejected_by_default() {
    let err = policy(
        ChecksumLookup::Unavailable("checksums.txt is malformed"),
        false,
    )
    .unwrap_err();
    assert!(
        matches!(err, DdlError::ChecksumUnavailable { .. }),
        "malformed checksum metadata must reject, got: {err}"
    );
}

#[test]
fn incomplete_metadata_asset_not_listed_is_rejected_by_default() {
    let err = policy(
        ChecksumLookup::Unavailable(
            "tool_v1.0.0_linux_amd64.tar.gz is not listed in this release's checksums.txt",
        ),
        false,
    )
    .unwrap_err();
    assert!(matches!(err, DdlError::ChecksumUnavailable { .. }));
}

#[test]
fn transport_failed_checksum_metadata_is_rejected_by_default() {
    let err = policy(
        ChecksumLookup::Unavailable("checksums.txt could not be fetched"),
        false,
    )
    .unwrap_err();
    assert!(matches!(err, DdlError::ChecksumUnavailable { .. }));
}

#[test]
fn valid_published_checksum_installs_verified() {
    let status = policy(ChecksumLookup::Published(FIXTURE_DIGEST.to_string()), false).unwrap();
    assert_eq!(status, VerificationStatus::Verified);
}

#[test]
fn checksum_mismatch_never_falls_back_to_cargo() {
    // The release publishes a digest that does NOT match the downloaded bytes.
    let wrong: &str = "d121be3103007b41edf96f8262925f8c7d61894afe9a041843b631f69445bc57";
    let err = policy(ChecksumLookup::Published(wrong.to_string()), false)
        .expect_err("wrong digest must reject");
    assert!(
        matches!(err, DdlError::ChecksumMismatch { .. }),
        "mismatch must surface as ChecksumMismatch (the only fallback trigger is NoReleaseBinary), got: {err}"
    );
    assert!(!matches!(err, DdlError::NoReleaseBinary { .. }));
}

#[test]
fn explicit_authorization_permits_unverified_install() {
    let status = policy(
        ChecksumLookup::Unavailable(
            "release publishes no checksum for tool_v1.0.0_linux_amd64.tar.gz",
        ),
        true,
    )
    .unwrap();
    assert_eq!(status, VerificationStatus::UnverifiedAuthorized);
}

#[test]
fn authorization_does_not_rescue_a_published_mismatch() {
    let other: &str = "d121be3103007b41edf96f8262925f8c7d61894afe9a041843b631f69445bc57";
    let err = policy(ChecksumLookup::Published(other.to_string()), true)
        .expect_err("explicit authorization must not bypass a published mismatch");
    assert!(matches!(err, DdlError::ChecksumMismatch { .. }));
}
