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
const INSTALL_ONLY: &[&str] = &["bd", "openspec", "incitaciones", "turu"];

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
    let (count, _rest) = line
        .strip_prefix("Managed tools (")
        .and_then(|l| l.split_once(')'))
        .expect("malformed managed tools line");
    let count: usize = count.parse().expect("managed tools count not a number");
    assert_eq!(
        count,
        MANAGED_TOOLS.len(),
        "README tool count drifts from MANAGED_TOOLS"
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

// ── Capability matrix (DDL-91a) ───────────────────────────────────────

/// docs/capability-matrix.md is generated, not hand-written. The committed
/// file must exactly match what the catalog module renders from the
/// registry — hand edits fail here until regenerated.
#[test]
fn capability_matrix_matches_generation() {
    let committed = include_str!("../docs/capability-matrix.md");
    let generated = dulce_de_leche::catalog::render_capability_matrix();
    assert_eq!(
        committed, generated,
        "docs/capability-matrix.md is out of sync with the registry — \
         regenerate it (see the catalog module) instead of hand-editing"
    );
}

// ── Registry partition (DDL-3i4) ──────────────────────────────────────

/// The draft partition from the usage census (family-evaluation §5), as
/// recorded in DDL-3i4. Category surfacing and future enforcement build on
/// this; changing the partition is a deliberate product decision, so the
/// expected mapping is locked here.
const EXPECTED_CATEGORY: &[(&str, &str)] = &[
    ("wai", "core"),
    ("testaruda", "core"),
    ("turu", "recommended"),
    ("dont", "recommended"),
    ("ah", "recommended"),
    ("pretender", "extension"),
    ("vampiro", "extension"),
    ("incitaciones", "extension"),
    ("bd", "extension"),
    ("openspec", "extension"),
    ("specodelic", "extension"),
];

#[test]
fn registry_partition_matches_census_draft() {
    assert_eq!(
        EXPECTED_CATEGORY.len(),
        MANAGED_TOOLS.len(),
        "partition table must cover every MANAGED_TOOLS entry"
    );
    for (name, category) in EXPECTED_CATEGORY {
        let tool = MANAGED_TOOLS
            .iter()
            .find(|t| t.name == *name)
            .unwrap_or_else(|| panic!("tool `{name}` in partition table not in registry"));
        assert_eq!(
            tool.category.as_str(),
            *category,
            "tool `{name}` category drifts from the census draft partition"
        );
    }
}

#[test]
fn every_tool_has_valid_maturity() {
    // §5 vocabulary: stable | beta | experimental | sunset (DDL-57e).
    const VALID: &[&str] = &["stable", "beta", "experimental", "sunset"];
    for tool in MANAGED_TOOLS {
        assert!(
            VALID.contains(&tool.maturity.as_str()),
            "tool `{}` has invalid maturity `{}`",
            tool.name,
            tool.maturity.as_str()
        );
    }
}

#[test]
fn maturity_matches_ratified_assignments() {
    // DDL-57e ratification: stable = frozen-API cores; beta = core works,
    // used in anger by ≥1 ecosystem tool; experimental = exploratory.
    let ratified: &[(&str, &str)] = &[
        ("wai", "stable"),
        ("dont", "beta"),
        ("ah", "beta"),
        ("pretender", "beta"),
        ("testaruda", "stable"),
        ("vampiro", "beta"),
        ("bd", "stable"),
        ("openspec", "beta"),
        ("incitaciones", "experimental"),
        ("turu", "beta"),
        ("specodelic", "experimental"),
    ];
    assert_eq!(
        ratified.len(),
        MANAGED_TOOLS.len(),
        "ratified map must cover the whole registry"
    );
    for (name, maturity) in ratified {
        let tool = MANAGED_TOOLS
            .iter()
            .find(|t| t.name == *name)
            .unwrap_or_else(|| panic!("tool `{name}` not in registry"));
        assert_eq!(
            tool.maturity.as_str(),
            *maturity,
            "tool `{name}` maturity drifts from the DDL-57e ratification"
        );
    }
}

#[test]
fn readme_category_blocks_match_registry() {
    let start = README
        .find("<!-- MANAGED-TOOLS:START -->")
        .expect("README.md must contain MANAGED-TOOLS markers");
    let end = README
        .find("<!-- MANAGED-TOOLS:END -->")
        .expect("README.md must contain MANAGED-TOOLS markers");
    let block = &README[start..end];

    let mut seen: Vec<&str> = Vec::new();
    for (heading, category) in [
        ("Core", "core"),
        ("Recommended", "recommended"),
        ("Extension", "extension"),
    ] {
        let prefix = format!("- {heading} (");
        let line = block
            .lines()
            .find(|l| l.starts_with(&prefix))
            .unwrap_or_else(|| panic!("README block missing `{heading}` category line"));
        let (count, list) = line[prefix.len()..]
            .split_once("): ")
            .unwrap_or_else(|| panic!("malformed `{heading}` category line"));
        let count: usize = count.parse().expect("category count not a number");
        let listed: Vec<&str> = list
            .trim_end_matches('.')
            .split(", ")
            .map(|s| s.trim())
            .collect();
        assert_eq!(
            listed.len(),
            count,
            "`{heading}` count does not match listed names"
        );
        for name in &listed {
            let tool = MANAGED_TOOLS
                .iter()
                .find(|t| t.name == *name)
                .unwrap_or_else(|| panic!("README lists `{name}` which is not in the registry"));
            assert_eq!(
                tool.category.as_str(),
                category,
                "README lists `{name}` under {heading} but registry says {}",
                tool.category.as_str()
            );
        }
        seen.extend(listed);
    }

    seen.sort();
    let mut expected = registry_names();
    expected.sort();
    assert_eq!(
        seen, expected,
        "README category blocks drift from MANAGED_TOOLS"
    );
}
