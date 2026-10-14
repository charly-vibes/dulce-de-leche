//! Integration tests for the ddl upgrade command (CLI parsing only).
//!
//! Upgrade actually runs subprocesses (brew, cargo, network) so we only
//! test CLI parsing and help text here. The upgrade logic is tested via
//! unit tests in the installer module.

mod common;
use assert_cmd::Command;
use predicates::prelude::*;
use std::time::Duration;

/// Timeout for each test.
const CMD_TIMEOUT: Duration = Duration::from_secs(10);

// ===================== CLI flag parsing tests =====================

#[test]
fn test_upgrade_help_has_tool_arg() {
    let mut cmd = Command::cargo_bin("ddl").unwrap();
    common::clean_git_env(&mut cmd);
    cmd.arg("upgrade").arg("--help");
    cmd.timeout(CMD_TIMEOUT);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("ddl upgrade"))
        .stdout(predicate::str::contains("TOOL"));
}

#[test]
fn test_upgrade_help_shows_description() {
    let mut cmd = Command::cargo_bin("ddl").unwrap();
    common::clean_git_env(&mut cmd);
    cmd.arg("upgrade").arg("--help");
    cmd.timeout(CMD_TIMEOUT);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Update all tools"));
}

#[test]
fn test_upgrade_unknown_tool_is_fast() {
    let (mut cmd, _temp) = ddl_cmd();
    cmd.arg("upgrade").arg("nonexistent-tool");
    cmd.timeout(CMD_TIMEOUT);
    // Unknown tool names are validated before any network/subprocess calls
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("not found").or(predicate::str::contains("error")));
}

#[test]
fn test_upgrade_unknown_flag_errors() {
    let mut cmd = Command::cargo_bin("ddl").unwrap();
    common::clean_git_env(&mut cmd);
    cmd.arg("upgrade").arg("--unknown-flag");
    cmd.timeout(CMD_TIMEOUT);
    cmd.assert().failure();
}

/// Helper: create a temp directory and return the cmd.
fn ddl_cmd() -> (Command, tempfile::TempDir) {
    let temp = tempfile::tempdir().unwrap();
    let mut cmd = Command::cargo_bin("ddl").unwrap();
    common::clean_git_env(&mut cmd);
    cmd.current_dir(temp.path());
    (cmd, temp)
}

// ===================== DDL-4oi: atomic upgrade (staged extract + replace) =====================
//
// Regression fixtures for SEC-01: an upgrade must never leave the installed
// executable partially written. Extraction goes to exclusive staging and the
// installed destination is only atomically replaced after the full, verified
// binary has landed there.

use std::fs;
use std::path::Path;

use dulce_de_leche::installer::stage_and_replace_binary;

const TOOL_NAME: &str = "fakebin";
const OLD_BYTES: &[u8] = b"#!/bin/sh\necho OLD-v1\n";
const NEW_BYTES: &[u8] = b"#!/bin/sh\necho NEW-v2\n";

/// Build a tar.gz archive containing `name -> data` regular-file entries.
fn tar_gz(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    let mut builder = tar::Builder::new(enc);
    for (name, data) in entries {
        let mut header = tar::Header::new_gnu();
        header.set_size(data.len() as u64);
        header.set_mode(0o755);
        header.set_entry_type(tar::EntryType::Regular);
        header.set_cksum();
        builder.append_data(&mut header, name, *data).unwrap();
    }
    builder.into_inner().unwrap().finish().unwrap()
}

/// Build a zip archive containing `name -> data` regular-file entries.
fn zip_bytes(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options: zip::write::SimpleFileOptions = Default::default();
    for (name, data) in entries {
        writer.start_file(*name, options).unwrap();
        std::io::Write::write_all(&mut writer, data).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

fn install_old_binary(dest: &Path) {
    fs::write(dest, OLD_BYTES).unwrap();
}

/// The destination directory must contain no abandoned staging directories
/// (`.ddl-stage-*`) after the call — success or failure.
fn assert_no_staging_leftovers(dest: &Path) {
    let parent = dest.parent().unwrap();
    let leftovers: Vec<_> = fs::read_dir(parent)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with(".ddl-stage"))
        .collect();
    assert!(
        leftovers.is_empty(),
        "abandoned staging entries in {}: {leftovers:?}",
        parent.display()
    );
}

#[test]
fn corrupt_tar_gz_preserves_installed_binary() {
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join(TOOL_NAME);
    install_old_binary(&dest);

    // Garbage that is not a valid archive at all.
    let corrupt: &[u8] = b"this is not a tar.gz archive, just noise\x01\x02\x03";
    let result = stage_and_replace_binary(corrupt, &dest, TOOL_NAME, "http://fixture/invalid");
    assert!(result.is_err(), "corrupt archive must be rejected");

    assert_eq!(
        fs::read(&dest).unwrap(),
        OLD_BYTES,
        "old binary must be byte-for-byte intact"
    );
    assert_no_staging_leftovers(&dest);
}

#[test]
fn truncated_tar_gz_preserves_installed_binary() {
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join(TOOL_NAME);
    install_old_binary(&dest);

    let full = tar_gz(&[(TOOL_NAME, NEW_BYTES)]);
    let truncated = &full[..full.len() / 2];
    let result = stage_and_replace_binary(truncated, &dest, TOOL_NAME, "http://fixture/truncated");
    assert!(result.is_err(), "truncated archive must be rejected");

    assert_eq!(
        fs::read(&dest).unwrap(),
        OLD_BYTES,
        "old binary must be byte-for-byte intact"
    );
    assert_no_staging_leftovers(&dest);
}

#[test]
fn interrupted_copy_preserves_installed_binary() {
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join(TOOL_NAME);
    install_old_binary(&dest);

    // The tar header declares 4096 bytes for the entry but only 16 are
    // present in the archive: the streaming copy into staging hits an
    // unexpected EOF mid-write, i.e. an interrupted copy.
    let enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    let mut builder = tar::Builder::new(enc);
    let mut header = tar::Header::new_gnu();
    header.set_size(4096);
    header.set_mode(0o755);
    header.set_entry_type(tar::EntryType::Regular);
    header.set_cksum();
    builder
        .append_data(&mut header, TOOL_NAME, NEW_BYTES)
        .unwrap();
    let lying_archive = builder.into_inner().unwrap().finish().unwrap();

    let result = stage_and_replace_binary(
        &lying_archive,
        &dest,
        TOOL_NAME,
        "http://fixture/interrupted",
    );
    assert!(
        result.is_err(),
        "entry shorter than its declared size must fail"
    );

    assert_eq!(
        fs::read(&dest).unwrap(),
        OLD_BYTES,
        "old binary must be byte-for-byte intact"
    );
    assert_no_staging_leftovers(&dest);
}

#[test]
fn symlink_entry_rejected_binary_preserved() {
    if !cfg!(unix) {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join(TOOL_NAME);
    install_old_binary(&dest);

    let enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    let mut builder = tar::Builder::new(enc);
    let mut header = tar::Header::new_gnu();
    header.set_entry_type(tar::EntryType::Symlink);
    header.set_mode(0o777);
    header.set_size(0);
    // link_name lives on the GNU/ustar extension; new_gnu supports it.
    header.set_cksum();
    builder
        .append_data(&mut header, TOOL_NAME, &[][..])
        .unwrap();
    let archive = builder.into_inner().unwrap().finish().unwrap();

    let result = stage_and_replace_binary(&archive, &dest, TOOL_NAME, "http://fixture/symlink");
    assert!(
        result.is_err(),
        "symlink entry must be rejected, never installed"
    );

    assert_eq!(
        fs::read(&dest).unwrap(),
        OLD_BYTES,
        "old binary must be byte-for-byte intact"
    );
    assert_no_staging_leftovers(&dest);
}

#[test]
fn directory_entry_rejected_binary_preserved() {
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join(TOOL_NAME);
    install_old_binary(&dest);

    let enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    let mut builder = tar::Builder::new(enc);
    let mut header = tar::Header::new_gnu();
    header.set_entry_type(tar::EntryType::Directory);
    header.set_mode(0o755);
    header.set_size(0);
    header.set_cksum();
    builder
        .append_data(&mut header, TOOL_NAME, &[][..])
        .unwrap();
    let archive = builder.into_inner().unwrap().finish().unwrap();

    let result = stage_and_replace_binary(&archive, &dest, TOOL_NAME, "http://fixture/directory");
    assert!(
        result.is_err(),
        "non-regular entry must be rejected, never installed"
    );

    assert_eq!(
        fs::read(&dest).unwrap(),
        OLD_BYTES,
        "old binary must be byte-for-byte intact"
    );
    assert_no_staging_leftovers(&dest);
}

#[test]
fn oversized_entry_rejected_binary_preserved() {
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join(TOOL_NAME);
    install_old_binary(&dest);

    let limit = dulce_de_leche::installer::MAX_BINARY_BYTES;
    let enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    let mut builder = tar::Builder::new(enc);
    let mut header = tar::Header::new_gnu();
    header.set_size(limit + 1);
    header.set_mode(0o755);
    header.set_entry_type(tar::EntryType::Regular);
    header.set_cksum();
    // Only the header is appended — the size bound must reject the entry from
    // its declared size alone, before any data is streamed.
    builder
        .append_data(&mut header, TOOL_NAME, &[][..])
        .unwrap();
    let archive = builder.into_inner().unwrap().finish().unwrap();

    let result = stage_and_replace_binary(&archive, &dest, TOOL_NAME, "http://fixture/oversized");
    assert!(
        result.is_err(),
        "entry exceeding the size bound must be rejected"
    );

    assert_eq!(
        fs::read(&dest).unwrap(),
        OLD_BYTES,
        "old binary must be byte-for-byte intact"
    );
    assert_no_staging_leftovers(&dest);
}

#[test]
fn successful_tar_gz_upgrade_replaces_and_cleans() {
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join(TOOL_NAME);
    install_old_binary(&dest);

    stage_and_replace_binary(
        &tar_gz(&[(TOOL_NAME, NEW_BYTES)]),
        &dest,
        TOOL_NAME,
        "http://fixture/ok",
    )
    .unwrap();

    assert_eq!(
        fs::read(&dest).unwrap(),
        NEW_BYTES,
        "replacement must be the new binary"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(&dest).unwrap().permissions().mode();
        assert_eq!(
            mode & 0o111,
            0o111,
            "replaced binary must remain executable"
        );
    }
    assert_no_staging_leftovers(&dest);
}

#[test]
fn corrupt_zip_preserves_installed_binary() {
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join(TOOL_NAME);
    install_old_binary(&dest);

    let result = stage_and_replace_binary(
        b"PK\x03\x04this-is-not-a-valid-zip-archive-truncated",
        &dest,
        TOOL_NAME,
        "http://fixture/badzip",
    );
    assert!(result.is_err(), "corrupt zip must be rejected");

    assert_eq!(
        fs::read(&dest).unwrap(),
        OLD_BYTES,
        "old binary must be byte-for-byte intact"
    );
    assert_no_staging_leftovers(&dest);
}

#[test]
fn successful_zip_upgrade_replaces_and_cleans() {
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join(TOOL_NAME);
    install_old_binary(&dest);

    stage_and_replace_binary(
        &zip_bytes(&[(TOOL_NAME, NEW_BYTES)]),
        &dest,
        TOOL_NAME,
        "http://fixture/okzip",
    )
    .unwrap();

    assert_eq!(
        fs::read(&dest).unwrap(),
        NEW_BYTES,
        "replacement must be the new binary"
    );
    assert_no_staging_leftovers(&dest);
}

#[test]
fn concurrent_upgrades_never_expose_partial_executable() {
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join(TOOL_NAME);
    install_old_binary(&dest);

    let archive_a = tar_gz(&[(TOOL_NAME, b"#!/bin/sh\necho VARIANT-A\n".as_slice())]);
    let archive_b = tar_gz(&[(TOOL_NAME, b"#!/bin/sh\necho VARIANT-B\n".as_slice())]);
    let dest_a = std::sync::Arc::new(dest.clone());
    let dest_b = dest_a.clone();

    let readers_running = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let reader_flag = readers_running.clone();
    let reader_dest = dest.clone();
    let reader = std::thread::spawn(move || {
        let mut samples = 0u32;
        while reader_flag.load(std::sync::atomic::Ordering::Relaxed) {
            if let Ok(bytes) = fs::read(&reader_dest) {
                assert!(
                    bytes == OLD_BYTES
                        || bytes == b"#!/bin/sh\necho VARIANT-A\n"
                        || bytes == b"#!/bin/sh\necho VARIANT-B\n",
                    "a concurrent reader must never observe a partial executable"
                );
                samples += 1;
            }
        }
        samples
    });

    let mut handles = Vec::new();
    for (i, archive) in [archive_a.clone(), archive_b.clone(), archive_a, archive_b]
        .into_iter()
        .enumerate()
    {
        let dest = dest_a.clone();
        handles.push(std::thread::spawn(move || {
            stage_and_replace_binary(&archive, &dest, TOOL_NAME, &format!("http://fixture/c{i}"))
        }));
    }
    let _ = dest_b;
    for handle in handles {
        handle.join().unwrap().unwrap();
    }

    readers_running.store(false, std::sync::atomic::Ordering::Relaxed);
    let samples = reader.join().unwrap();
    assert!(samples > 0, "reader must have sampled at least once");

    // Final state: a complete, executable payload — never a mix.
    let final_bytes = fs::read(&dest).unwrap();
    assert!(
        final_bytes == b"#!/bin/sh\necho VARIANT-A\n"
            || final_bytes == b"#!/bin/sh\necho VARIANT-B\n",
        "final executable must be one complete variant"
    );
    assert_no_staging_leftovers(&dest);
}

// ===================== DDL-1z1 (SEC-02): bootstrap resource budgets =====================
//
// Regression fixtures for the bootstrap resource-budget policy: release
// downloads, captured child output and child runtime are bounded by explicit
// numeric budgets, and an exhausted budget is identified in diagnostics
// instead of silently falling back to a less restricted install method.

use std::time::Instant;

use dulce_de_leche::installer::{
    MAX_CHILD_OUTPUT_BYTES, MAX_RELEASE_ASSET_BYTES, enforce_download_budget, run_child_captured,
};

#[test]
fn download_budget_rejects_oversize_declared_length() {
    // A release body declaring more than the download budget is rejected
    // before any byte crosses the budget.
    let err = enforce_download_budget(Some(MAX_RELEASE_ASSET_BYTES + 1), 0).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("download"),
        "message must name the budget: {msg}"
    );
    assert!(msg.contains("budget"), "message must say budget: {msg}");
}

#[test]
fn download_budget_rejects_oversize_streamed_bytes() {
    // Unknown declared length: the streamed byte count is still bounded.
    let err = enforce_download_budget(None, MAX_RELEASE_ASSET_BYTES + 1).unwrap_err();
    assert!(
        err.to_string().contains("download"),
        "message must name the download budget: {err}"
    );
}

#[test]
fn download_budget_accepts_within_cap() {
    enforce_download_budget(Some(1024), 1024).unwrap();
    enforce_download_budget(None, 0).unwrap();
}

#[cfg(unix)]
#[test]
fn hanging_child_killed_within_deadline_plus_one() {
    let mut cmd = std::process::Command::new("sleep");
    cmd.arg("60");
    let start = Instant::now();
    let err = run_child_captured(&mut cmd, Duration::from_secs(1)).unwrap_err();
    let elapsed = start.elapsed();
    assert!(
        err.to_string().contains("budget"),
        "timeout must be reported as a budget exhaustion: {err}"
    );
    assert!(
        elapsed < Duration::from_secs(2),
        "child must be terminated within deadline + 1s; took {elapsed:?}"
    );
}

#[cfg(unix)]
#[test]
fn hanging_child_with_descendants_killed_within_deadline_plus_one() {
    // `sh -c 'sleep 60'` makes the sleeper a *descendant* of the direct
    // child; both must be gone when the deadline fires.
    let mut cmd = std::process::Command::new("sh");
    cmd.arg("-c").arg("sleep 60");
    let start = Instant::now();
    let err = run_child_captured(&mut cmd, Duration::from_secs(1)).unwrap_err();
    let elapsed = start.elapsed();
    assert!(err.to_string().contains("budget"), "{err}");
    assert!(
        elapsed < Duration::from_secs(2),
        "child tree must be terminated within deadline + 1s; took {elapsed:?}"
    );
    // No orphaned sleeper may survive the group kill.
    std::thread::sleep(Duration::from_millis(100));
    let mut pgrep = std::process::Command::new("sh");
    pgrep
        .arg("-c")
        .arg("ps -eo args= | grep -F 'sleep 60' | grep -v grep");
    let check = pgrep.output().unwrap();
    assert!(
        check.stdout.is_empty(),
        "descendants survived the runtime budget kill: {:?}",
        String::from_utf8_lossy(&check.stdout)
    );
}

#[cfg(unix)]
#[test]
fn runaway_captured_output_rejected() {
    // A child emitting one byte more than the captured-output budget must be
    // rejected — the budget names itself in the error.
    let script = format!("head -c {} /dev/zero", MAX_CHILD_OUTPUT_BYTES as u64 + 1);
    let mut cmd = std::process::Command::new("sh");
    cmd.arg("-c").arg(&script);
    let err = run_child_captured(&mut cmd, Duration::from_secs(10)).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("captured-output"),
        "message must name the captured-output budget: {msg}"
    );
    assert!(msg.contains("budget"), "message must say budget: {msg}");
}

#[cfg(unix)]
#[test]
fn captured_output_within_budget_succeeds() {
    let mut cmd = std::process::Command::new("sh");
    cmd.arg("-c").arg("echo hello");
    let out = run_child_captured(&mut cmd, Duration::from_secs(5)).unwrap();
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "hello");
}
