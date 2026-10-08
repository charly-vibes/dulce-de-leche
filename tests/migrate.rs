//! Integration tests for the `migrate` command — symlink farm, undo, single-file configs.
//!
//! Tests cover:
//! - DDL-el9: `migrate` corrupts single-file configs (`.pretender.toml`)
//! - DDL-cbp: `migrate --undo` creates directory instead of file for single-file configs
//! - DDL-0yp: Normalize migration paths so `migrate --undo` detects symlinks

use serial_test::serial;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

use dulce_de_leche::dot_ddl::DdlDir;

/// Helper: create a temp dir with a `.ddl/` directory and return both.
fn setup_ddl_dir() -> (TempDir, DdlDir) {
    let tmp = TempDir::new().expect("create temp dir");
    let ddl_dir = DdlDir::create_at(tmp.path().join(".ddl").as_path()).expect("create .ddl dir");
    (tmp, ddl_dir)
}

/// Helper: create a single-file legacy config at the given path.
fn create_single_file_legacy(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create parent dir");
    }
    std::fs::write(path, content).expect("write legacy config");
}

/// Helper: create a directory legacy config at the given path.
fn create_directory_legacy(path: &Path, files: &[(&str, &str)]) {
    std::fs::create_dir_all(path).expect("create legacy dir");
    for (name, content) in files {
        std::fs::write(path.join(name), content).expect("write legacy file");
    }
}

/// Helper: change to a temp dir, run a closure, then restore the original CWD.
/// Uses a drop guard to restore CWD even on panic.
/// Needed because `migrated_tools()` constructs paths from CWD-relative LEGACY_CONFIGS.
fn with_cwd(dir: &Path, f: impl FnOnce()) {
    let original = std::env::current_dir().expect("get current dir");
    struct CwdGuard(PathBuf);
    impl Drop for CwdGuard {
        fn drop(&mut self) {
            let _ = std::env::set_current_dir(&self.0);
        }
    }
    let _guard = CwdGuard(original);
    std::env::set_current_dir(dir).expect("set current dir");
    f();
}

// ===== DDL-el9: migrate corrupts single-file configs =====

#[test]
fn test_migrate_single_file_config_creates_correct_symlink() {
    let (tmp, ddl_dir) = setup_ddl_dir();

    // Create a legacy single-file config (.pretender.toml) in the temp dir
    let legacy_path = tmp.path().join(".pretender.toml");
    create_single_file_legacy(&legacy_path, r#"theme = "dark""#);

    // Migrate it
    ddl_dir
        .migrate_tool("pretender", &legacy_path)
        .expect("migrate single-file config");

    // After migration, the legacy path should be a symlink
    assert!(
        legacy_path.is_symlink(),
        "legacy path should be a symlink after migration"
    );

    // Read the symlink target
    let target = std::fs::read_link(&legacy_path).expect("read symlink target");
    let expected = ddl_dir.tool_path("pretender").join(".pretender.toml");
    assert_eq!(
        target, expected,
        "symlink should point to the file inside .ddl/, not the directory"
    );

    // The file content should be accessible through the symlink
    let content = std::fs::read_to_string(&legacy_path).expect("read through symlink");
    assert_eq!(
        content, r#"theme = "dark""#,
        "content accessible through symlink"
    );
}

#[test]
fn test_migrate_directory_config_creates_correct_symlink() {
    let (tmp, ddl_dir) = setup_ddl_dir();

    // Create a legacy directory config (.wai)
    let legacy_path = tmp.path().join(".wai");
    create_directory_legacy(&legacy_path, &[("config.toml", "key = true")]);

    // Migrate it
    ddl_dir
        .migrate_tool("wai", &legacy_path)
        .expect("migrate directory config");

    // After migration, the legacy path should be a symlink
    assert!(
        legacy_path.is_symlink(),
        "legacy path should be a symlink after migration"
    );

    // Read the symlink target — should point to the directory
    let target = std::fs::read_link(&legacy_path).expect("read symlink target");
    let expected = ddl_dir.tool_path("wai");
    assert_eq!(
        target, expected,
        "directory symlink should point to .ddl/<tool>/"
    );
}

// ===== DDL-cbp: migrate --undo creates directory instead of file =====

#[test]
fn test_migrate_undo_single_file_restores_as_file() {
    let (tmp, ddl_dir) = setup_ddl_dir();

    let legacy_path = tmp.path().join(".pretender.toml");
    create_single_file_legacy(&legacy_path, r#"theme = "light""#);

    // Migrate forward
    ddl_dir
        .migrate_tool("pretender", &legacy_path)
        .expect("migrate single-file config");

    // Undo
    ddl_dir
        .unmigrate_tool("pretender", &legacy_path)
        .expect("undo migration");

    // After undo, legacy path should be a regular file, not a directory
    assert!(legacy_path.exists(), "legacy path should exist after undo");
    assert!(
        legacy_path.is_file(),
        "legacy path should be a file, not a directory, after undo"
    );

    // Content should be restored
    let content = std::fs::read_to_string(&legacy_path).expect("read restored file");
    assert_eq!(content, r#"theme = "light""#, "content restored correctly");
}

#[test]
fn test_migrate_undo_directory_restores_as_directory() {
    let (tmp, ddl_dir) = setup_ddl_dir();

    let legacy_path = tmp.path().join(".wai");
    create_directory_legacy(
        &legacy_path,
        &[("config.toml", "key = true"), ("notes.md", "hello")],
    );

    // Migrate forward
    ddl_dir
        .migrate_tool("wai", &legacy_path)
        .expect("migrate directory config");

    // Undo
    ddl_dir
        .unmigrate_tool("wai", &legacy_path)
        .expect("undo migration");

    // After undo, legacy path should be a directory
    assert!(
        legacy_path.is_dir(),
        "legacy path should be a directory after undo"
    );

    // Files should be restored
    assert!(
        legacy_path.join("config.toml").exists(),
        "config.toml should be restored"
    );
    assert!(
        legacy_path.join("notes.md").exists(),
        "notes.md should be restored"
    );
}

// ===== DDL-0yp: Normalize migration paths for symlink detection =====

#[test]
#[serial]
fn test_migrated_tools_detects_relative_symlinks() {
    let (tmp, ddl_dir) = setup_ddl_dir();

    // Change to the temp dir so that CWD-relative LEGACY_CONFIGS paths resolve
    with_cwd(tmp.path(), || {
        // Create a legacy config in the temp dir (relative: ".wai")
        let legacy_path = Path::new(".wai");
        create_directory_legacy(legacy_path, &[("config.toml", "key = true")]);

        // Migrate — this creates a relative symlink (.ddl/wai/)
        ddl_dir
            .migrate_tool("wai", legacy_path)
            .expect("migrate directory config");

        // Reconstruct DdlDir from the same .ddl/
        let ddl_dir2 = DdlDir::create_at(Path::new(".ddl")).expect("recreate DdlDir");

        // migrated_tools should detect the symlink regardless of relative/absolute
        let migrated = dulce_de_leche::dot_ddl::migrated_tools(&ddl_dir2);
        let wai = migrated.iter().find(|(name, _)| name == "wai");
        assert!(
            wai.is_some(),
            "migrated_tools should detect wai even with relative symlink target"
        );
    });
}

#[test]
#[serial]
fn test_migrated_tools_detects_single_file_symlinks() {
    let (tmp, ddl_dir) = setup_ddl_dir();

    with_cwd(tmp.path(), || {
        // Create a single-file legacy config (relative: ".pretender.toml")
        let legacy_path = Path::new(".pretender.toml");
        create_single_file_legacy(legacy_path, r#"theme = "dark""#);

        // Migrate forward
        ddl_dir
            .migrate_tool("pretender", legacy_path)
            .expect("migrate single-file config");

        // Reconstruct DdlDir
        let ddl_dir2 = DdlDir::create_at(Path::new(".ddl")).expect("recreate DdlDir");

        // migrated_tools should detect the single-file symlink
        let migrated = dulce_de_leche::dot_ddl::migrated_tools(&ddl_dir2);
        let pretender = migrated.iter().find(|(name, _)| name == "pretender");
        assert!(
            pretender.is_some(),
            "migrated_tools should detect single-file config symlink"
        );
    });
}

#[test]
fn test_migrated_tools_empty_when_none_migrated() {
    let (_tmp, ddl_dir) = setup_ddl_dir();

    // No legacy configs exist, so nothing should be detected
    let migrated = dulce_de_leche::dot_ddl::migrated_tools(&ddl_dir);
    assert!(
        migrated.is_empty(),
        "no migrated tools should be detected when none exist"
    );
}

// ===== Round-trip: migrate forward then undo produces identical state =====

#[test]
fn test_migrate_round_trip_single_file() {
    let (tmp, ddl_dir) = setup_ddl_dir();

    let legacy_path = tmp.path().join(".pretender.toml");
    let original_content = r#"theme = "roundtrip""#;
    create_single_file_legacy(&legacy_path, original_content);

    // Migrate forward
    ddl_dir
        .migrate_tool("pretender", &legacy_path)
        .expect("migrate single-file config");

    // Undo
    ddl_dir
        .unmigrate_tool("pretender", &legacy_path)
        .expect("undo migration");

    // After round-trip, the file should be restored exactly
    assert!(legacy_path.is_file(), "should be a file after round-trip");
    let content = std::fs::read_to_string(&legacy_path).expect("read restored file");
    assert_eq!(
        content, original_content,
        "content preserved through round-trip"
    );
}

#[test]
fn test_migrate_round_trip_directory() {
    let (tmp, ddl_dir) = setup_ddl_dir();

    let legacy_path = tmp.path().join(".wai");
    create_directory_legacy(
        &legacy_path,
        &[
            ("config.toml", "key = true"),
            ("profile.toml", "mode = dev"),
        ],
    );

    // Migrate forward
    ddl_dir
        .migrate_tool("wai", &legacy_path)
        .expect("migrate directory config");

    // Undo
    ddl_dir
        .unmigrate_tool("wai", &legacy_path)
        .expect("undo migration");

    // After round-trip, the directory should be restored
    assert!(
        legacy_path.is_dir(),
        "should be a directory after round-trip"
    );
    assert!(
        legacy_path.join("config.toml").exists(),
        "config.toml should exist after round-trip"
    );
    assert!(
        legacy_path.join("profile.toml").exists(),
        "profile.toml should exist after round-trip"
    );
}
// ===== DDL-40e: migrate is idempotent =====

#[test]
fn test_migrate_idempotent_directory() {
    let (tmp, ddl_dir) = setup_ddl_dir();

    let legacy_path = tmp.path().join(".wai");
    create_directory_legacy(&legacy_path, &[("config.toml", "key = true")]);

    // First migration
    ddl_dir
        .migrate_tool("wai", &legacy_path)
        .expect("first migrate");

    // Second migration — should be a no-op
    ddl_dir
        .migrate_tool("wai", &legacy_path)
        .expect("second migrate (idempotent)");

    // Symlink should still point to the correct target
    assert!(legacy_path.is_symlink(), "should still be a symlink");
    let target = std::fs::read_link(&legacy_path).expect("read symlink");
    let expected = ddl_dir.tool_path("wai");
    assert_eq!(target, expected, "symlink target should be unchanged");

    // Content should be accessible
    assert!(
        legacy_path.join("config.toml").exists(),
        "config.toml should be accessible after second migrate"
    );
}

#[test]
fn test_migrate_idempotent_single_file() {
    let (tmp, ddl_dir) = setup_ddl_dir();

    let legacy_path = tmp.path().join(".pretender.toml");
    create_single_file_legacy(&legacy_path, r#"theme = "idempotent""#);

    // First migration
    ddl_dir
        .migrate_tool("pretender", &legacy_path)
        .expect("first migrate");

    // Second migration — should be a no-op
    ddl_dir
        .migrate_tool("pretender", &legacy_path)
        .expect("second migrate (idempotent)");

    // Symlink should still point to the correct target
    assert!(legacy_path.is_symlink(), "should still be a symlink");
    let target = std::fs::read_link(&legacy_path).expect("read symlink");
    let expected = ddl_dir.tool_path("pretender").join(".pretender.toml");
    assert_eq!(target, expected, "symlink target should be unchanged");

    // Content should be accessible
    let content = std::fs::read_to_string(&legacy_path).expect("read through symlink");
    assert_eq!(content, r#"theme = "idempotent""#);
}

// ===== DDL-6zn.7: unmigrate_all (bulk --undo) is non-destructive =====

#[test]
#[serial]
fn test_unmigrate_all_preserves_state_json_verbatim() {
    let (tmp, ddl_dir) = setup_ddl_dir();

    // migrated_tools() resolves LEGACY_CONFIGS relative to CWD
    with_cwd(tmp.path(), || {
        // Migrate two tools: wai (directory config) and pretender (single-file config)
        let wai_path = tmp.path().join(".wai");
        create_directory_legacy(&wai_path, &[("config.toml", "key = true")]);
        ddl_dir.migrate_tool("wai", &wai_path).expect("migrate wai");

        let pretender_path = tmp.path().join(".pretender.toml");
        create_single_file_legacy(&pretender_path, r#"theme = "light""#);
        ddl_dir
            .migrate_tool("pretender", &pretender_path)
            .expect("migrate pretender");

        // Write a custom state.json into the wai tool dir — this is what
        // unmigrate_all must NOT delete
        let state_path = ddl_dir.tool_path("wai").join("state.json");
        let state_content = r#"{"plugins":["core","flow"],"last_sync":"2026-02-14T10:00:00Z"}"#;
        std::fs::write(&state_path, state_content).expect("write state.json");

        // Run the bulk undo
        let restored = ddl_dir.unmigrate_all().expect("unmigrate_all");

        assert_eq!(restored.len(), 2, "both tools should be restored");
        assert!(restored.contains(&("wai".to_string(), wai_path.clone())));
        assert!(restored.contains(&("pretender".to_string(), pretender_path.clone())));

        // state.json must survive verbatim
        assert!(
            state_path.exists(),
            "state.json should not be deleted by unmigrate_all"
        );
        let content = std::fs::read_to_string(&state_path).expect("read state.json");
        assert_eq!(
            content, state_content,
            "state.json content must be preserved verbatim"
        );

        // Legacy configs restored correctly
        assert!(wai_path.is_dir(), "wai legacy dir restored");
        assert!(pretender_path.is_file(), "pretender legacy file restored");
    });
}

#[test]
#[serial]
fn test_unmigrate_all_skips_state_json_when_restoring() {
    let (tmp, ddl_dir) = setup_ddl_dir();

    with_cwd(tmp.path(), || {
        // Even when the legacy dir contains state.json originally (it was symlinked
        // in), unmigrate_all must not delete it from the .ddl/<tool>/ side after restore.
        let wai_path = tmp.path().join(".wai");
        create_directory_legacy(
            &wai_path,
            &[
                ("config.toml", "key = true"),
                ("state.json", r#"{"plugins":["core"]}"#),
            ],
        );
        ddl_dir.migrate_tool("wai", &wai_path).expect("migrate wai");

        // Overwrite state.json in the .ddl side to simulate an update since migration
        let state_path = ddl_dir.tool_path("wai").join("state.json");
        let updated = r#"{"plugins":["core","flow"]}"#;
        std::fs::write(&state_path, updated).expect("update state.json in .ddl side");

        let restored = ddl_dir.unmigrate_all().expect("unmigrate_all");
        assert_eq!(restored.len(), 1);

        // The .ddl side must still hold its copy — unmigrate_all only restores
        // the symlink, it does not touch the backing store.
        assert!(
            state_path.exists(),
            "state.json in .ddl/<tool>/ must survive unmigrate_all"
        );
        let content = std::fs::read_to_string(&state_path).expect("read state.json");
        assert_eq!(content, updated, "state.json unchanged in .ddl side");

        // Legacy dir restored with its original content (via symlink restore)
        let legacy_state = wai_path.join("state.json");
        let legacy_content =
            std::fs::read_to_string(&legacy_state).expect("read legacy state.json");
        assert_eq!(
            legacy_content, updated,
            "legacy path sees updated state.json"
        );
    });
}
