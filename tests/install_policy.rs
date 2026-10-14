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

// ============================================================================
// DDL-5bs (SEC-04): replay tool installations from exact versions and
// artifact identities. Fixture notes:
//   - The replay path takes the lock ONLY — there is no latest-version input,
//     so zero latest resolution holds by construction.
//   - Time is explicit (`*_epoch_day`); no test touches the wall clock.
//   - The lock is a separate file/schema from the manifest inventory.
// ============================================================================

mod replay_lock {
    use dulce_de_leche::installer::{
        ReleaseCandidate, UpgradeDecision, cargo_replay_args, npm_replay_args, replay_decision,
        upgrade_decision,
    };
    use dulce_de_leche::manifest::{InstallLock, LockEntry, Manifest};

    const URL_V1: &str =
        "https://github.com/charly-vibes/wai/releases/download/v2026.10.3/wai-x86_64.tar.gz";
    const URL_V2: &str =
        "https://github.com/charly-vibes/wai/releases/download/v2026.11.0/wai-x86_64.tar.gz";
    const SHA_V1: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const SHA_V2: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    fn recorded(day: u64) -> LockEntry {
        LockEntry {
            tool: "wai".into(),
            version: "2026.10.3".into(),
            artifact_url: URL_V1.into(),
            sha256: SHA_V1.into(),
            recorded_epoch_day: day,
        }
    }

    fn candidate(version: &str, url: &str, sha: &str, published: u64) -> ReleaseCandidate {
        ReleaseCandidate {
            version: version.into(),
            artifact_url: url.into(),
            sha256: sha.into(),
            published_epoch_day: published,
        }
    }

    #[test]
    fn locked_replay_selects_recorded_identity_even_when_latest_changes() {
        let mut lock = InstallLock::new();
        lock.record(recorded(20_470));

        // `releases/latest` moved to a newer release between record and replay.
        // The replay path has no latest input at all — the decision depends
        // only on the lock, so the newer release cannot leak in.
        let entry = replay_decision(&lock, "wai").expect("replay must resolve from the lock");
        assert_eq!(entry.version, "2026.10.3");
        assert_eq!(entry.artifact_url, URL_V1);
        assert_eq!(entry.sha256, SHA_V1);
        assert_eq!(entry.recorded_epoch_day, 20_470);

        // Two locked replays are identical even as "latest" drifts.
        let again = replay_decision(&lock, "wai").expect("second replay");
        assert_eq!(entry, again);
    }

    #[test]
    fn replay_without_recorded_identity_fails_with_offline_semantics() {
        let lock = InstallLock::new();
        let err = replay_decision(&lock, "wai")
            .expect_err("replay must never fall back to floating latest");
        match &err {
            dulce_de_leche::error::DdlError::InstallFailed(msg) => {
                assert!(msg.contains("wai"), "diagnostic names the tool: {msg}");
                assert!(
                    msg.contains("ddl install"),
                    "diagnostic instructs recording an identity: {msg}"
                );
                assert!(
                    !msg.to_lowercase().contains("latest"),
                    "offline semantics must not suggest latest resolution: {msg}"
                );
            }
            other => panic!("expected InstallFailed, got {other:?}"),
        }
    }

    #[test]
    fn cargo_replay_pins_exact_version_and_locked_dependencies() {
        let args = cargo_replay_args("wai", "2026.10.3");
        let joined = args.join(" ");
        assert!(args.contains(&"--version".to_string()));
        assert!(args.contains(&"2026.10.3".to_string()));
        assert!(args.contains(&"--locked".to_string()));
        assert!(!joined.contains("latest"));
    }

    #[test]
    fn npm_replay_uses_selected_package_version() {
        let args = npm_replay_args("@fission-ai/openspec", "0.2.1");
        assert_eq!(args, vec!["install", "-g", "@fission-ai/openspec@0.2.1"]);
        assert!(!args.join(" ").contains("@latest"));
    }

    #[test]
    fn explicit_upgrade_reports_resolved_changes() {
        let lock = InstallLock::new();
        let mut lock = lock;
        lock.record(recorded(20_470));
        let decision = upgrade_decision(
            lock.get("wai").expect("recorded"),
            &candidate("2026.11.0", URL_V2, SHA_V2, 20_500),
            0, // no cool-down
            20_507,
        )
        .expect("upgrade decision");
        match decision {
            UpgradeDecision::Upgrade { from, to } => {
                assert_eq!(from.version, "2026.10.3");
                assert_eq!(from.sha256, SHA_V1);
                assert_eq!(to.version, "2026.11.0");
                assert_eq!(to.artifact_url, URL_V2);
                assert_eq!(to.sha256, SHA_V2);
            }
            other => panic!("expected Upgrade, got {other:?}"),
        }
    }

    #[test]
    fn release_age_policy_defers_fresh_releases_deterministically() {
        let mut lock = InstallLock::new();
        lock.record(recorded(20_470));
        let cand = candidate("2026.11.0", URL_V2, SHA_V2, 20_500);

        // Published 20_500, today 20_502 → age 2 < 7 → deferred, lock intact.
        let decision = upgrade_decision(lock.get("wai").unwrap(), &cand, 7, 20_502).unwrap();
        match decision {
            UpgradeDecision::Deferred { reason } => {
                assert!(reason.contains("7"), "reason names the policy: {reason}");
            }
            other => panic!("expected Deferred, got {other:?}"),
        }

        // Today 20_507 → age 7 ≥ 7 → allowed. Deterministic in `today`.
        let decision = upgrade_decision(lock.get("wai").unwrap(), &cand, 7, 20_507).unwrap();
        assert!(matches!(decision, UpgradeDecision::Upgrade { .. }));
    }

    #[test]
    fn identical_candidate_is_up_to_date() {
        let mut lock = InstallLock::new();
        lock.record(recorded(20_470));
        let same = candidate("2026.10.3", URL_V1, SHA_V1, 20_400);
        let decision = upgrade_decision(lock.get("wai").unwrap(), &same, 0, 20_507).unwrap();
        assert!(matches!(decision, UpgradeDecision::UpToDate));
    }

    #[test]
    fn lock_file_round_trips_atomically_and_leaves_no_temp_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("lock.json");
        let mut lock = InstallLock::new();
        lock.record(recorded(20_470));
        lock.save(&path).expect("save");

        let reloaded = InstallLock::load(&path).expect("load");
        assert_eq!(reloaded, lock);

        let leftovers: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(Result::ok)
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".tmp"))
            .collect();
        assert!(
            leftovers.is_empty(),
            "no temp files left behind: {leftovers:?}"
        );
    }

    #[test]
    fn absent_lock_file_loads_as_empty_and_inventory_stays_distinct() {
        let dir = tempfile::tempdir().unwrap();
        let lock = InstallLock::load(&dir.path().join("lock.json")).expect("absent → empty");
        assert!(lock.get("wai").is_none());

        // The inventory manifest keeps its own schema — a legacy manifest
        // still parses and shares no structure with the lock.
        let legacy = r#"{"ddl_version":"0.7.0","migration_state":"none","tools":{}}"#;
        let m = Manifest::parse(legacy).expect("legacy manifest unchanged");
        assert!(m.tools.is_empty());
    }
}
