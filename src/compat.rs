//! Version compatibility matrix — dynamic fetch with embedded fallback.
//!
//! The compatibility matrix defines which tool versions are compatible with
//! the current version of ddl. It is fetched dynamically from
//! `https://charly-vibes.github.io/ddl/compatibility.json` at runtime,
//! with an embedded fallback for offline use and a local cache.

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{DdlError, Result};

/// The compatibility matrix — maps tool names to version constraints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompatibilityMatrix {
    /// ddl version that this matrix was built for.
    #[serde(default)]
    pub ddl_version: String,
    /// Per-tool version constraints (e.g., ">=2026.3.0").
    pub tools: HashMap<String, String>,
}

impl CompatibilityMatrix {
    /// URL where the dynamic matrix is hosted.
    pub const REMOTE_URL: &'static str = "https://charly-vibes.github.io/ddl/compatibility.json";

    /// Fetch the matrix from the remote URL.
    pub fn fetch() -> Result<Self> {
        let response = reqwest::blocking::get(Self::REMOTE_URL).map_err(DdlError::Network)?;

        if !response.status().is_success() {
            return Err(DdlError::Network(response.error_for_status().unwrap_err()));
        }

        let matrix: Self = response
            .json()
            .map_err(|e| DdlError::Other(format!("Failed to parse compatibility matrix: {e}")))?;

        Ok(matrix)
    }

    /// Load the embedded fallback matrix. Mirrors
    /// `manifest::EMBEDDED_COMPATIBILITY` exactly — keep in sync
    /// (enforced by tests::test_embedded_matrix_matches_manifest_embedded_json).
    pub fn embedded() -> Self {
        let tools: HashMap<String, String> = [
            ("wai", ">=2026.3.0"),
            ("dont", ">=0.2.0"),
            ("ah", ">=0.2.0"),
            ("pretender", ">=0.3.0"),
            ("sad", ">=0.1.0"),
            ("testaruda", ">=0.2.0"),
            ("vampiro", ">=0.4.0"),
            ("incitaciones", ">=0.8.0"),
            ("turu", ">=0.3.0"),
            ("specodelic", ">=0.1.0"),
            ("funkalization", ">=0.1.0"),
            ("bd", ">=0.0.0"),
            ("openspec", ">=0.0.0"),
            ("wild", ">=0.0.0"),
            ("bajan", ">=0.1.0"),
            ("podras", ">=0.0.0"),
            ("kaya", ">=0.1.0"),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();

        Self {
            ddl_version: crate::VERSION.to_string(),
            tools,
        }
    }

    /// Load the matrix from a local cache file.
    pub fn from_cache(path: &Path) -> Option<Self> {
        let contents = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&contents).ok()
    }

    /// Save the matrix to a local cache file.
    pub fn save_cache(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(DdlError::Io)?;
        }
        let contents = serde_json::to_string_pretty(self).map_err(DdlError::Serde)?;
        std::fs::write(path, contents).map_err(DdlError::Io)?;
        Ok(())
    }

    /// Get the version constraint for a tool.
    pub fn constraint(&self, tool_name: &str) -> Option<&str> {
        self.tools.get(tool_name).map(|s| s.as_str())
    }

    /// Check if a version satisfies the constraint for a tool.
    pub fn is_compatible(&self, tool_name: &str, version: &str) -> bool {
        let constraint = match self.tools.get(tool_name) {
            Some(c) => c,
            None => return true, // no constraint = compatible
        };

        let ver = match semver::Version::parse(version.trim_start_matches('v')) {
            Ok(v) => v,
            Err(_) => return true, // can't parse = assume compatible
        };

        let req = match semver::VersionReq::parse(constraint) {
            Ok(r) => r,
            Err(_) => return true, // can't parse = assume compatible
        };

        req.matches(&ver)
    }

    /// Get the latest version of a tool from the matrix (if known).
    /// This is a placeholder — the matrix only stores constraints, not latest versions.
    pub fn latest_versions(&self) -> HashMap<String, String> {
        // TODO: fetch latest versions from GitHub API
        HashMap::new()
    }
}

/// Load the compatibility matrix with fallback: cache → remote → embedded.
/// Returns the matrix and any warnings (e.g., failed network fetch).
pub fn load_compatibility(ddl_dir: &Path) -> (CompatibilityMatrix, Vec<String>) {
    load_compatibility_from(ddl_dir, CompatibilityMatrix::fetch)
}

/// Fetch-injectable core of [`load_compatibility`] (unit-testable offline).
/// Resolution order: remote fetch → local cache → embedded fallback.
pub(crate) fn load_compatibility_from(
    ddl_dir: &Path,
    fetch: impl FnOnce() -> Result<CompatibilityMatrix>,
) -> (CompatibilityMatrix, Vec<String>) {
    let cache_path = ddl_dir.join("compatibility-cache.json");
    let mut warnings = Vec::new();

    // Try remote fetch first
    match fetch() {
        Ok(matrix) => {
            // Save to cache
            let _ = matrix.save_cache(&cache_path);
            return (matrix, warnings);
        }
        Err(e) => {
            warnings.push(format!("Could not fetch compatibility matrix: {e}"));
        }
    }

    // Try cache
    if let Some(matrix) = CompatibilityMatrix::from_cache(&cache_path) {
        return (matrix, warnings);
    }

    // Fall back to embedded
    (CompatibilityMatrix::embedded(), warnings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::DdlError;

    fn matrix_with(pairs: &[(&str, &str)]) -> CompatibilityMatrix {
        CompatibilityMatrix {
            ddl_version: "test".to_string(),
            tools: pairs
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        }
    }

    #[test]
    fn test_embedded_matrix_matches_manifest_embedded_json() {
        // keep-in-sync contract: compat::embedded() must mirror
        // manifest::EMBEDDED_COMPATIBILITY exactly.
        let manifest_tools: std::collections::HashMap<String, String> =
            serde_json::from_str(crate::manifest::EMBEDDED_COMPATIBILITY).unwrap();
        let compat = CompatibilityMatrix::embedded();
        assert_eq!(
            compat.tools, manifest_tools,
            "compat::embedded and manifest::EMBEDDED_COMPATIBILITY drifted"
        );
    }

    #[test]
    fn test_embedded_matrix_constraints_are_parseable() {
        let embedded = CompatibilityMatrix::embedded();
        assert!(
            !embedded.tools.is_empty(),
            "embedded fallback matrix must not be empty"
        );
        for (tool, constraint) in &embedded.tools {
            semver::VersionReq::parse(constraint).unwrap_or_else(|e| {
                panic!("constraint {constraint:?} for {tool} unparseable: {e}")
            });
        }
        // The pinned core must always be present.
        for tool in ["wai", "dont", "ah", "pretender", "testaruda", "vampiro"] {
            assert!(
                embedded.constraint(tool).is_some(),
                "embedded matrix missing core tool {tool}"
            );
        }
    }

    #[test]
    fn test_compatible_version_within_range_installs() {
        let m = matrix_with(&[("wai", ">=2026.3.0")]);
        assert!(m.is_compatible("wai", "2026.11.17"));
        assert!(
            m.is_compatible("wai", "v2026.3.0"),
            "v-prefix must be tolerated"
        );
    }

    #[test]
    fn test_incompatible_version_outside_range_is_refused() {
        let m = matrix_with(&[("wai", ">=2026.3.0")]);
        assert!(!m.is_compatible("wai", "2026.2.0"));
    }

    #[test]
    fn test_matrix_missing_tool_is_treated_as_compatible() {
        let m = matrix_with(&[]);
        assert!(m.is_compatible("wai", "0.0.1"));
    }

    #[test]
    fn test_cache_roundtrip_preserves_constraints() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("compatibility-cache.json");
        let m = matrix_with(&[("wai", ">=2026.3.0"), ("dont", ">=0.2.0")]);
        m.save_cache(&path).unwrap();
        let loaded = CompatibilityMatrix::from_cache(&path).expect("cache must reload");
        assert_eq!(loaded.constraint("wai"), Some(">=2026.3.0"));
        assert_eq!(loaded.constraint("dont"), Some(">=0.2.0"));
    }

    #[test]
    fn test_load_falls_back_to_cache_when_fetch_fails() {
        let temp = tempfile::tempdir().unwrap();
        let ddl_dir = temp.path().to_path_buf();
        let cached = matrix_with(&[("wai", ">=2026.3.0")]);
        cached
            .save_cache(&ddl_dir.join("compatibility-cache.json"))
            .unwrap();
        let (matrix, warnings) =
            load_compatibility_from(&ddl_dir, || Err(DdlError::Other("offline".to_string())));
        assert_eq!(
            matrix.constraint("wai"),
            Some(">=2026.3.0"),
            "cache must win after failed fetch"
        );
        assert!(
            warnings
                .iter()
                .any(|w| w.contains("Could not fetch compatibility matrix")),
            "failed fetch must warn, got {warnings:?}"
        );
    }

    #[test]
    fn test_load_falls_back_to_embedded_when_fetch_and_cache_fail() {
        let temp = tempfile::tempdir().unwrap();
        let (matrix, warnings) =
            load_compatibility_from(temp.path(), || Err(DdlError::Other("offline".to_string())));
        assert!(
            !matrix.tools.is_empty(),
            "embedded fallback must provide constraints"
        );
        assert_eq!(warnings.len(), 1);
    }
}
