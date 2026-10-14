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
