//! Concurrency tests for manifest updates — DDL-h1l.
//!
//! Tests that concurrent manifest updates don't lose entries by using
//! file locking to serialize read-modify-write cycles.

use std::sync::{Arc, Barrier};
use tempfile::TempDir;

use dulce_de_leche::{dot_ddl::DdlDir, manifest::Manifest};

/// Helper: create a temp dir with a `.ddl/` directory.
fn setup_ddl_dir() -> (TempDir, DdlDir) {
    let tmp = TempDir::new().expect("create temp dir");
    let ddl_dir = DdlDir::create_at(tmp.path().join(".ddl").as_path()).expect("create .ddl dir");
    (tmp, ddl_dir)
}

#[test]
fn test_concurrent_manifest_updates_preserve_all_entries() {
    let (_tmp, _ddl_dir) = setup_ddl_dir();
    let ddl_path = _tmp.path().join(".ddl");

    // Use a barrier to synchronize two threads so they both load the
    // manifest before either one writes. Without locking, the second
    // write would overwrite the first.
    let barrier = Arc::new(Barrier::new(2));
    let path1 = ddl_path.clone();
    let path2 = ddl_path.clone();
    let b1 = Arc::clone(&barrier);
    let b2 = Arc::clone(&barrier);

    let handle1 = std::thread::spawn(move || {
        let mut dd = DdlDir::create_at(&path1).expect("create DdlDir for thread 1");
        b1.wait(); // both threads have loaded; write now
        dd.record_installed("tool-a", "1.0.0", "cargo")
            .expect("record tool-a");
    });

    let handle2 = std::thread::spawn(move || {
        let mut dd = DdlDir::create_at(&path2).expect("create DdlDir for thread 2");
        b2.wait(); // both threads have loaded; write now
        dd.record_installed("tool-b", "2.0.0", "cargo")
            .expect("record tool-b");
    });

    // Wait for completion
    handle1.join().expect("thread 1 panicked");
    handle2.join().expect("thread 2 panicked");

    // Verify both entries survived the concurrent writes
    let dd = DdlDir::create_at(&ddl_path).expect("reload DdlDir");
    assert!(
        dd.manifest.is_installed("tool-a"),
        "tool-a should be in manifest after concurrent writes"
    );
    assert!(
        dd.manifest.is_installed("tool-b"),
        "tool-b should be in manifest after concurrent writes"
    );
    assert_eq!(
        dd.manifest.tools.len(),
        2,
        "manifest should have exactly 2 tools, not 1"
    );
}

#[test]
fn test_concurrent_manifest_update_with_serial_writes() {
    // Without concurrency, serial writes should work fine.
    // This is a baseline sanity check.
    let (_tmp, mut dd) = setup_ddl_dir();

    dd.record_installed("tool-a", "1.0.0", "cargo")
        .expect("record tool-a");
    dd.record_installed("tool-b", "2.0.0", "cargo")
        .expect("record tool-b");

    assert!(dd.manifest.is_installed("tool-a"));
    assert!(dd.manifest.is_installed("tool-b"));
    assert_eq!(dd.manifest.tools.len(), 2);
}

// === SEC-01 (DDL-8u8): durability across interrupted/concurrent updates ===
//
// Baseline evidence: `write_manifest_to` truncated the locked data file
// before writing, so an interrupted save left an empty (or partial JSON)
// manifest — which `Manifest::parse("")` then accepted as a fresh manifest,
// silently resetting established tool state.

/// Injected failure (read-only `.ddl/` directory) must abort the save
/// *before* replacing the manifest, leaving the prior valid manifest
/// byte-for-byte intact.
///
/// The injection is `#[cfg(unix)]`: on Unix, staging a replacement file
/// requires write permission on the directory, so the failure lands in the
/// staging step. The assertion that a failed staging attempt must error —
/// never fall back to truncate-then-write — is the regression this pins
/// down; on Windows the directory read-only attribute does not block file
/// creation, so the injection cannot be reproduced there.
#[cfg(unix)]
#[test]
fn test_failed_save_leaves_prior_manifest_byte_for_byte_intact() {
    use std::os::unix::fs::PermissionsExt;

    let tmp = TempDir::new().expect("create temp dir");
    let ddl_dir = tmp.path().join(".ddl");
    let mut dd = DdlDir::create_at(&ddl_dir).expect("create .ddl dir");
    dd.record_installed("tool-a", "1.0.0", "cargo")
        .expect("record tool-a");

    let manifest_path = ddl_dir.join("manifest.json");
    let prior = std::fs::read(&manifest_path).expect("read prior manifest");

    // Inject: make `.ddl/` non-writable so the atomic staging step fails.
    let mut perms = std::fs::metadata(&ddl_dir)
        .expect("stat .ddl")
        .permissions();
    let original_mode = perms.mode();
    perms.set_mode(0o555);
    std::fs::set_permissions(&ddl_dir, perms).expect("chmod .ddl read-only");

    let result = dd.record_installed("tool-b", "2.0.0", "cargo");

    // Restore permissions before any assertion so TempDir cleanup works.
    let mut perms = std::fs::metadata(&ddl_dir)
        .expect("stat .ddl")
        .permissions();
    perms.set_mode(original_mode);
    std::fs::set_permissions(&ddl_dir, perms).expect("restore .ddl permissions");

    // A save that cannot be staged atomically must fail rather than fall
    // back to truncate-then-write on the live manifest.
    assert!(
        result.is_err(),
        "save must fail when the manifest cannot be staged atomically"
    );

    let after = std::fs::read(&manifest_path).expect("read manifest after failure");
    assert_eq!(
        prior, after,
        "injected failure before replacement must leave the prior manifest byte-for-byte intact"
    );

    // Established state is not silently reset by the failed save.
    let dd = DdlDir::create_at(&ddl_dir).expect("reload .ddl dir");
    assert!(dd.manifest.is_installed("tool-a"), "tool-a must survive");
    assert!(
        !dd.manifest.is_installed("tool-b"),
        "tool-b must not appear"
    );
    assert_eq!(dd.manifest.tools.len(), 1);
}

/// During repeated replacements, concurrent readers must always observe a
/// complete manifest: full JSON that parses, and either the complete old or
/// the complete new state — never an empty or partially-written file.
///
/// A reader that observes 0 tools (truncation artifact accepted by the old
/// `parse("") -> new manifest` path) or fails to parse (partial JSON) fails
/// this fixture.
#[test]
fn test_readers_never_observe_partial_manifest_during_replacement() {
    use std::sync::atomic::{AtomicBool, Ordering};

    let tmp = TempDir::new().expect("create temp dir");
    let ddl_dir = tmp.path().join(".ddl");
    let mut dd = DdlDir::create_at(&ddl_dir).expect("create .ddl dir");
    dd.record_installed("tool-x", "1.0.0", "cargo")
        .expect("record tool-x");
    let manifest_path = ddl_dir.join("manifest.json");

    let stop = Arc::new(AtomicBool::new(false));
    let mut readers = Vec::new();
    for _ in 0..3 {
        let path = manifest_path.clone();
        let stop = Arc::clone(&stop);
        readers.push(std::thread::spawn(move || {
            let mut observations = 0usize;
            while !stop.load(Ordering::Relaxed) {
                let manifest =
                    Manifest::load(&path).expect("manifest must always parse completely");
                assert_eq!(
                    manifest.tools.len(),
                    1,
                    "every observed manifest must be a complete state, not a truncation artifact"
                );
                let entry = manifest
                    .get_tool("tool-x")
                    .expect("tool-x must be present in every complete state");
                assert!(
                    entry.installed == "1.0.0" || entry.installed == "2.0.0",
                    "observed version must be the complete old or complete new value"
                );
                observations += 1;
            }
            observations
        }));
    }

    let writer_path = ddl_dir.clone();
    let writer = std::thread::spawn(move || {
        let mut dd = DdlDir::create_at(&writer_path).expect("create DdlDir for writer");
        for i in 0..300 {
            let version = if i % 2 == 0 { "2.0.0" } else { "1.0.0" };
            dd.record_installed("tool-x", version, "cargo")
                .expect("record tool-x");
        }
    });
    writer.join().expect("writer panicked");

    stop.store(true, Ordering::Relaxed);
    for reader in readers {
        let observations = reader.join().expect("reader panicked");
        assert!(
            observations > 0,
            "readers must have observed at least one complete manifest"
        );
    }
}
