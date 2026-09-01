//! Drift detection between the tool registry and the documentation.
//!
//! Fails when `MANAGED_TOOLS`, `EMBEDDED_COMPATIBILITY`, the legacy config
//! mappings, or README.md's managed-tools block disagree.
//!
//! Added after vampiro v0.4.0 shipped without being added to the registry —
//! the registry, docs, and ecosystem had drifted apart silently.

use dulce_de_leche::dot_ddl::legacy_configs;
use dulce_de_leche::manifest::EMBEDDED_COMPATIBILITY;
use dulce_de_leche::platform::MANAGED_TOOLS;

const README: &str = include_str!("../README.md");
const ECOSYSTEM_MAP: &str = include_str!("../docs/ecosystem-map.md");

/// Tools that are install-only (no legacy config to migrate).
const INSTALL_ONLY: &[&str] = &["fotos-mcp"];

fn registry_names() -> Vec<&'static str> {
    MANAGED_TOOLS.iter().map(|t| t.name).collect()
}

#[test]
fn readme_managed_tools_block_matches_registry() {
    let start = README
        .find("<!-- MANAGED-TOOLS:START -->")
        .expect("README.md must contain MANAGED-TOOLS markers — see tests/tool_registry_drift.rs");
    let end = README
        .find("<!-- MANAGED-TOOLS:END -->")
        .expect("README.md must contain MANAGED-TOOLS markers — see tests/tool_registry_drift.rs");
    let block = &README[start..end];

    let line = block
        .lines()
        .find(|l| l.starts_with("Managed tools ("))
        .expect("managed tools line missing from README block");
    let (count, list) = line
        .strip_prefix("Managed tools (")
        .and_then(|l| l.split_once("): "))
        .expect("malformed managed tools line");
    let count: usize = count.parse().expect("managed tools count not a number");
    assert_eq!(
        count,
        MANAGED_TOOLS.len(),
        "README tool count drifts from MANAGED_TOOLS"
    );

    let mut listed: Vec<&str> = list
        .trim_end_matches('.')
        .split(", ")
        .map(|s| s.trim())
        .collect();
    listed.sort();
    let mut expected = registry_names();
    expected.sort();
    assert_eq!(
        listed, expected,
        "README managed-tools block drifts from MANAGED_TOOLS"
    );
}

#[test]
fn embedded_compatibility_covers_registry_exactly() {
    let matrix: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(EMBEDDED_COMPATIBILITY)
            .expect("EMBEDDED_COMPATIBILITY must be valid JSON");
    for name in registry_names() {
        assert!(
            matrix.contains_key(name),
            "tool `{name}` missing from EMBEDDED_COMPATIBILITY"
        );
    }
    assert_eq!(
        matrix.len(),
        MANAGED_TOOLS.len(),
        "EMBEDDED_COMPATIBILITY has entries not in MANAGED_TOOLS"
    );
}

#[test]
fn every_tool_except_install_only_has_legacy_config() {
    for name in registry_names() {
        let has_entry = legacy_configs().iter().any(|(n, _)| *n == name);
        if INSTALL_ONLY.contains(&name) {
            assert!(
                !has_entry,
                "`{name}` is install-only but has a legacy config mapping"
            );
        } else {
            assert!(
                has_entry,
                "tool `{name}` lacks a legacy config mapping in dot_ddl.rs"
            );
        }
    }
}

#[test]
fn ecosystem_map_mentions_every_tool() {
    for name in registry_names() {
        assert!(
            ECOSYSTEM_MAP.contains(name),
            "tool `{name}` missing from docs/ecosystem-map.md"
        );
    }
}
