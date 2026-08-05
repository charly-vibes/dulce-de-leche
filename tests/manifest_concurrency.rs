//! Concurrency tests for manifest updates — DDL-h1l.
//!
//! Tests that concurrent manifest updates don't lose entries by using
//! file locking to serialize read-modify-write cycles.

use std::sync::{Arc, Barrier};
use tempfile::TempDir;

use dulce_de_leche::dot_ddl::DdlDir;

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
