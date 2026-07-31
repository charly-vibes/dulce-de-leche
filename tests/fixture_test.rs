//! Tests using genesis::fixture::Fixture for scaffold/init-like scenarios.
//!
//! Demonstrates adoption of genesis::fixture and genesis::config.

use genesis::config::ConfigFile;
use genesis::fixture::Fixture;

/// Verify that genesis Fixture works for testing ddl config scenarios.
#[test]
fn test_ddl_config_via_fixture() {
    let fixture = Fixture::new()
        .with_marker(".ddl")
        .build()
        .expect("build fixture");

    // Write a DdlConfig via genesis ConfigFile.
    // `write()` takes the "repo root" that ConfigFile::path() joins to.
    // DdlConfig::path() joins "config.toml", so passing `.ddl/` writes
    // to `.ddl/config.toml`.
    let config = dulce_de_leche::DdlConfig {
        install_source: "cargo".to_string(),
        auto_upgrade: true,
        gitignore_strategy: "auto".to_string(),
    };
    config
        .write(fixture.root().join(".ddl").as_path())
        .expect("write config");

    // Verify the file exists and is valid TOML
    fixture.assert_file_exists(".ddl/config.toml");
    fixture.assert_file_contains(".ddl/config.toml", "cargo");
    fixture.assert_file_contains(".ddl/config.toml", "auto_upgrade");

    // Round-trip via ConfigFile::read
    let read = dulce_de_leche::DdlConfig::read(fixture.root().join(".ddl").as_path())
        .expect("read config back");
    assert_eq!(read.install_source, "cargo");
    assert!(read.auto_upgrade);
    assert_eq!(read.gitignore_strategy, "auto");
}

/// Verify genesis Fixture with managed block markers.
#[test]
fn test_agents_block_generation() {
    let block = dulce_de_leche::agents_block();
    assert!(block.contains("<!-- DDL:START -->"));
    assert!(block.contains("<!-- DDL:END -->"));
    assert!(block.contains("dulce-de-leche"));
    assert!(block.contains("ddl init"));
    assert!(block.contains("ddl status"));
}
