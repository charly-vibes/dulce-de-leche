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

// =====================================================================
// DDL-hsn (SEC-03 / CORR-002): networked-mock END-TO-END regression for
// the checksum-mismatch abort.
//
// Fixture separation (CO-MOD): everything below is self-contained — a
// std-only HTTP mock plus two subprocess tests driving the REAL `ddl`
// binary against it. No shared state with the unit-policy tests above.
//
// Must criteria exercised here (against a local mock, no new deps):
//   * mismatched download → nonzero exit, destination untouched,
//     zero cargo fallback;
//   * the valid counterpart (same shape, honest digest) installs
//     verified.
// =====================================================================
mod checksum_mismatch_e2e {
    use assert_cmd::Command;
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use sha2::{Digest, Sha256};
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;
    use tempfile::TempDir;

    const TOOL: &str = "wai";
    const REPO: &str = "charly-vibes/wai";
    const TAG: &str = "v9.9.9";
    const ARCHIVE_NAME: &str = "wai_9.9.9_linux_amd64.tar.gz";

    fn sha256_hex(bytes: &[u8]) -> String {
        let digest = Sha256::digest(bytes);
        let mut hex = String::with_capacity(64);
        for byte in digest {
            hex.push_str(&format!("{byte:02x}"));
        }
        hex
    }

    /// A minimal tar.gz whose member is named exactly like the tool, so
    /// `extract_tar_gz` finds it and the installer writes it to `~/.ddl/bin`.
    fn fake_archive() -> Vec<u8> {
        let payload = b"#!/bin/sh\necho ddl-hsn-fake-wai\n";
        let mut builder = tar::Builder::new(Vec::new());
        let mut header = tar::Header::new_gnu();
        header.set_size(payload.len() as u64);
        header.set_mode(0o755);
        header.set_cksum();
        builder
            .append_data(&mut header, TOOL, &payload[..])
            .unwrap();
        builder.finish().unwrap();
        let tar_bytes = builder.into_inner().unwrap();
        let mut gz = GzEncoder::new(Vec::new(), Compression::default());
        gz.write_all(&tar_bytes).unwrap();
        gz.finish().unwrap()
    }

    /// DDL-wof provenance manifest naming ddl's own trust roots (imported
    /// from the lib — never re-hardcoded here), so the valid counterpart
    /// passes identity checks and only the digest can be the discriminator
    /// between the two scenarios.
    fn provenance_json(digest: &str) -> String {
        use dulce_de_leche::installer::{
            EXPECTED_PROVENANCE_REPOSITORY, EXPECTED_PROVENANCE_WORKFLOW,
        };
        format!(
            "{{\"repository\":\"{EXPECTED_PROVENANCE_REPOSITORY}\",\"workflow\":\"{EXPECTED_PROVENANCE_WORKFLOW}\",\"tag\":\"{TAG}\",\"artifacts\":{{\"{ARCHIVE_NAME}\":\"{digest}\"}}}}"
        )
    }

    /// Minimal std-only HTTP/1.1 mock serving the GitHub-release surface
    /// (latest-release lookup, asset, checksums.txt, provenance.json).
    /// Every served request path is sent over `requests` for assertions.
    fn spawn_mock(
        checksums: String,
        provenance: String,
        archive: Vec<u8>,
    ) -> (String, mpsc::Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { break };
                let mut buf = [0u8; 8192];
                let mut data = Vec::new();
                loop {
                    match stream.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            data.extend_from_slice(&buf[..n]);
                            if data.windows(4).any(|w| w == b"\r\n\r\n") {
                                break;
                            }
                        }
                    }
                }
                let first_line = data
                    .split(|b| *b == b'\n')
                    .next()
                    .map(|l| String::from_utf8_lossy(l).into_owned())
                    .unwrap_or_default();
                let path = first_line
                    .split_whitespace()
                    .nth(1)
                    .unwrap_or("")
                    .to_string();
                let _ = tx.send(path.clone());
                let stripped = path.split('?').next().unwrap_or("");
                let (ctype, body): (&str, Vec<u8>) = if stripped.contains("/releases/latest") {
                    (
                        "application/json",
                        format!("{{\"tag_name\":\"{TAG}\"}}").into_bytes(),
                    )
                } else if stripped.ends_with("/checksums.txt") {
                    ("text/plain", checksums.clone().into_bytes())
                } else if stripped.ends_with("/provenance.json") {
                    ("application/json", provenance.clone().into_bytes())
                } else {
                    ("application/gzip", archive.clone())
                };
                let head = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(head.as_bytes());
                let _ = stream.write_all(&body);
                let _ = stream.flush();
            }
        });
        (format!("http://127.0.0.1:{port}"), rx)
    }

    /// Run the real `ddl` binary in a hermetic sandbox: temp HOME (so the
    /// install destination is isolated), temp CWD (so the .ddl manifest
    /// dir is isolated), a stripped PATH (no preinstalled managed tools,
    /// and structurally zero cargo available), and proxy vars removed so
    /// the mock endpoint is always reached directly.
    fn ddl_cmd(mock_base: &str, home: &TempDir, cwd: &TempDir) -> Command {
        let mut cmd = Command::cargo_bin("ddl").unwrap();
        cmd.arg("install")
            .arg(TOOL)
            .arg("--yes")
            .env("DDL_TEST_RELEASE_BASE_URL", mock_base)
            .env("HOME", home.path())
            .env("PATH", "/usr/bin:/bin")
            .env_remove("HTTP_PROXY")
            .env_remove("http_proxy")
            .env_remove("HTTPS_PROXY")
            .env_remove("https_proxy")
            .env_remove("ALL_PROXY")
            .env_remove("all_proxy")
            .current_dir(cwd.path())
            .timeout(Duration::from_secs(30));
        cmd
    }

    /// Drain every request the mock served during the child's run.
    fn served_requests(rx: &mpsc::Receiver<String>) -> Vec<String> {
        let mut served = Vec::new();
        while let Ok(path) = rx.try_recv() {
            served.push(path);
        }
        served
    }

    fn dest_path(home: &TempDir) -> std::path::PathBuf {
        home.path().join(".ddl").join("bin").join(TOOL)
    }

    /// SEC-03: a mismatched download must abort the install end-to-end —
    /// nonzero exit, destination never written, and NO cargo fallback —
    /// when the release's own checksums.txt disagrees with the asset.
    #[test]
    fn mismatched_download_aborts_without_installing() {
        if !matches!(
            (std::env::consts::OS, std::env::consts::ARCH),
            ("linux", "x86_64")
        ) {
            return; // fixture archive is built for the linux_amd64 target
        }
        let archive = fake_archive();
        let honest_digest = sha256_hex(&archive);
        let attacker_digest = "0".repeat(64); // well-formed, matches nothing
        let (base, rx) = spawn_mock(
            format!("{attacker_digest}  {ARCHIVE_NAME}\n"),
            provenance_json(&honest_digest),
            archive.clone(),
        );

        let home = TempDir::new().unwrap();
        let cwd = TempDir::new().unwrap();
        let output = ddl_cmd(&base, &home, &cwd).output().unwrap();
        let combined = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        assert!(
            !output.status.success(),
            "mismatched download must exit nonzero; output:\n{combined}"
        );
        assert!(
            combined.contains(&format!("Checksum verification failed for {TOOL}")),
            "abort must be reported as a checksum rejection; output:\n{combined}"
        );
        assert!(
            !dest_path(&home).exists(),
            "destination must be untouched when the download is rejected"
        );
        assert!(
            !combined.contains("cargo install"),
            "checksum mismatch must not fall back to cargo; output:\n{combined}"
        );

        // The mock must have actually served the whole lookup chain —
        // this is what distinguishes a routed mock run from an accident.
        let served = served_requests(&rx);
        assert!(
            served
                .iter()
                .any(|p| p.contains(&format!("/repos/{REPO}/releases/latest"))),
            "mock never saw the release lookup; requests: {served:?}"
        );
        assert!(
            served.iter().any(|p| p.ends_with("/checksums.txt")),
            "mock never saw the checksums.txt lookup; requests: {served:?}"
        );
        assert!(
            served.iter().any(|p| p.ends_with(ARCHIVE_NAME)),
            "mock never served the asset; requests: {served:?}"
        );
    }

    /// The valid counterpart of the mismatch scenario: same shape, honest
    /// digest in checksums.txt and provenance.json → install succeeds,
    /// destination holds the extracted binary, and the run reports the
    /// sha256-verified status.
    #[test]
    fn matching_download_installs_verified() {
        if !matches!(
            (std::env::consts::OS, std::env::consts::ARCH),
            ("linux", "x86_64")
        ) {
            return; // fixture archive is built for the linux_amd64 target
        }
        let archive = fake_archive();
        let digest = sha256_hex(&archive);
        let (base, rx) = spawn_mock(
            format!("{digest}  {ARCHIVE_NAME}\n"),
            provenance_json(&digest),
            archive.clone(),
        );

        let home = TempDir::new().unwrap();
        let cwd = TempDir::new().unwrap();
        let output = ddl_cmd(&base, &home, &cwd).output().unwrap();
        let combined = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        assert!(
            output.status.success(),
            "valid counterpart must install successfully; output:\n{combined}"
        );
        assert!(
            combined.contains("sha256 verified"),
            "success must report the sha256-verified status; output:\n{combined}"
        );
        let dest = dest_path(&home);
        assert!(
            dest.exists(),
            "installed binary must exist at the destination"
        );
        let installed = std::fs::read(&dest).unwrap();
        assert_eq!(
            installed,
            b"#!/bin/sh\necho ddl-hsn-fake-wai\n".to_vec(),
            "destination content must be the archive member, not the archive"
        );

        let served = served_requests(&rx);
        assert!(
            served.iter().any(|p| p.ends_with("/checksums.txt")),
            "valid install must consult checksums.txt; requests: {served:?}"
        );
        assert!(
            served.iter().any(|p| p.ends_with("/provenance.json")),
            "valid install must consult provenance.json (DDL-wof); requests: {served:?}"
        );
    }
}
