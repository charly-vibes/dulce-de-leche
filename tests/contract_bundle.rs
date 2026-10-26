//! Contract bundle verification and base caching (wild-xgd).
//!
//! Red-first spec source: openspec change `add-wild-contract-bundle-convention`
//! (wild repo) §2 tasks 2.1–2.2 — the bundle refusal law (`bundle-digest-mismatch`,
//! `bundle-commit-mismatch`, named diagnostics, never silent acceptance) and the
//! base-contract cache (`contracts/<tool>/<version>/` in the ddl state dir,
//! digest recorded, missing/corrupt → `missing-base-contract`, never fabricated).

use dulce_de_leche::contract_bundle::{
    VerifiedBundle, cache_base_bundle, load_cached_base, verify_bundle,
};
use sha2::{Digest, Sha256};
use std::path::Path;

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

const CONTRACT_JSON: &str = r#"{"kind":"contract","slots":[]}"#;

/// Materialize a bundle directory with the given manifest and contract bytes.
fn write_bundle(dir: &Path, manifest: &str, contract: &[u8]) {
    std::fs::create_dir_all(dir).expect("create bundle dir");
    std::fs::write(dir.join("manifest.toml"), manifest).expect("write manifest");
    std::fs::write(dir.join("contract.json"), contract).expect("write contract");
}

fn valid_manifest(contract: &[u8]) -> String {
    format!(
        "tool = 'testaruda'\nversion = '0.5.7'\ncommit = 'abc1234'\nprofile = 'rust-cargo-local-1'\ncontract_sha256 = '{}'\nextractor_version = 'wild 0.2.0'\nextracted_at = '2026-10-21T12:00:00Z'\n",
        sha256_hex(contract)
    )
}

// ── 2.1 refusal law ────────────────────────────────────────────────────────

#[test]
fn valid_bundle_verifies_and_returns_manifest_and_contract_bytes() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path().join("testaruda-0.5.7-contract-bundle");
    let contract = CONTRACT_JSON.as_bytes();
    write_bundle(&dir, &valid_manifest(contract), contract);

    let verified = verify_bundle(&dir, "abc1234").expect("valid bundle must verify");
    assert_eq!(verified.manifest.tool, "testaruda");
    assert_eq!(verified.manifest.version, "0.5.7");
    assert_eq!(verified.manifest.commit, "abc1234");
    assert_eq!(verified.manifest.profile, "rust-cargo-local-1");
    assert_eq!(
        verified.contract_sha256,
        sha256_hex(contract),
        "the verified digest must be the recomputed one"
    );
    assert_eq!(verified.contract_json, contract);
}

#[test]
fn digest_mismatch_is_refused_with_named_diagnostic() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path().join("bundle");
    // Manifest stamps the digest of DIFFERENT bytes than the shipped contract.
    let manifest = valid_manifest(b"different bytes");
    write_bundle(&dir, &manifest, CONTRACT_JSON.as_bytes());

    let err = verify_bundle(&dir, "abc1234").expect_err("must refuse");
    assert!(
        err.to_string().contains("bundle-digest-mismatch"),
        "refusal must name the diagnostic, got: {err}"
    );
}

#[test]
fn commit_mismatch_is_refused_with_named_diagnostic() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path().join("bundle");
    let contract = CONTRACT_JSON.as_bytes();
    write_bundle(&dir, &valid_manifest(contract), contract);

    let err = verify_bundle(&dir, "def5678").expect_err("commit mismatch must refuse");
    assert!(
        err.to_string().contains("bundle-commit-mismatch"),
        "refusal must name the diagnostic, got: {err}"
    );
}

#[test]
fn missing_manifest_or_contract_is_refused_not_accepted() {
    let tmp = tempfile::tempdir().expect("tempdir");

    let empty = tmp.path().join("empty");
    std::fs::create_dir_all(&empty).expect("mkdir");
    let err = verify_bundle(&empty, "abc1234").expect_err("empty bundle must refuse");
    assert!(
        err.to_string().contains("bundle-malformed"),
        "missing files must refuse with the named diagnostic, got: {err}"
    );

    // Manifest present but contract missing.
    let partial = tmp.path().join("partial");
    std::fs::create_dir_all(&partial).expect("mkdir");
    std::fs::write(partial.join("manifest.toml"), "tool = 'x'\n").expect("write");
    let err = verify_bundle(&partial, "abc1234").expect_err("missing contract must refuse");
    assert!(
        err.to_string().contains("bundle-malformed"),
        "missing contract must refuse with the named diagnostic, got: {err}"
    );
}

#[test]
fn bundle_with_extra_files_is_refused() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path().join("bundle");
    let contract = CONTRACT_JSON.as_bytes();
    write_bundle(&dir, &valid_manifest(contract), contract);
    std::fs::write(dir.join("extra.txt"), "sneaky").expect("write extra");

    let err = verify_bundle(&dir, "abc1234").expect_err("extra files must refuse");
    assert!(
        err.to_string().contains("bundle-malformed"),
        "unexpected bundle contents must refuse, got: {err}"
    );
}

#[test]
fn malformed_manifest_is_refused_not_accepted() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path().join("bundle");
    write_bundle(&dir, "not toml at all [[[", CONTRACT_JSON.as_bytes());

    let err = verify_bundle(&dir, "abc1234").expect_err("malformed manifest must refuse");
    assert!(err.to_string().contains("bundle-malformed"), "got: {err}");
}

// ── 2.2 base-contract cache ────────────────────────────────────────────────

#[test]
fn cached_base_bundle_round_trips_under_state_dir() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let state_dir = tmp.path().join("state");
    let bundle = tmp.path().join("bundle");
    let contract = CONTRACT_JSON.as_bytes();
    write_bundle(&bundle, &valid_manifest(contract), contract);
    let verified: VerifiedBundle = verify_bundle(&bundle, "abc1234").expect("verify");

    cache_base_bundle(&state_dir, "testaruda", "0.5.7", &verified).expect("cache");

    let cached_dir = state_dir.join("contracts/testaruda/0.5.7");
    assert!(cached_dir.join("contract.json").is_file());
    assert!(cached_dir.join("manifest.toml").is_file());

    let loaded = load_cached_base(&state_dir, "testaruda", "0.5.7", &verified.contract_sha256)
        .expect("cached base must load");
    assert_eq!(loaded.contract_sha256, verified.contract_sha256);
    assert_eq!(loaded.contract_json, contract);
}

#[test]
fn corrupt_cached_base_refuses_with_named_diagnostic_not_silent_skip() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let state_dir = tmp.path().join("state");
    let bundle = tmp.path().join("bundle");
    let contract = CONTRACT_JSON.as_bytes();
    write_bundle(&bundle, &valid_manifest(contract), contract);
    let verified = verify_bundle(&bundle, "abc1234").expect("verify");
    cache_base_bundle(&state_dir, "testaruda", "0.5.7", &verified).expect("cache");

    // Corrupt the cached contract behind the recorded digest.
    std::fs::write(
        state_dir.join("contracts/testaruda/0.5.7/contract.json"),
        b"tampered",
    )
    .expect("tamper");

    let err = load_cached_base(&state_dir, "testaruda", "0.5.7", &verified.contract_sha256)
        .expect_err("corrupt base must not load");
    assert!(
        err.to_string().contains("cached-base-digest-mismatch"),
        "got: {err}"
    );
}

#[test]
fn missing_cached_base_yields_missing_base_contract_not_fabrication() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let state_dir = tmp.path().join("state");

    let err = load_cached_base(&state_dir, "testaruda", "0.5.7", "deadbeef")
        .expect_err("missing base must not fabricate");
    assert!(
        err.to_string().contains("missing-base-contract"),
        "got: {err}"
    );
}
