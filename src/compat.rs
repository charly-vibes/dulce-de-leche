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
        let response =
            reqwest::blocking::get(Self::REMOTE_URL).map_err(|e| DdlError::Network(e))?;

        if !response.status().is_success() {
            return Err(DdlError::Network(response.error_for_status().unwrap_err()));
        }

        let matrix: Self = response
            .json()
            .map_err(|e| DdlError::Other(format!("Failed to parse compatibility matrix: {e}")))?;

        Ok(matrix)
    }

    /// Load the embedded fallback matrix.
    pub fn embedded() -> Self {
        let tools: HashMap<String, String> = [
            ("wai", ">=2026.3.0"),
            ("dont", ">=0.2.0"),
            ("ah", ">=0.2.0"),
            ("pretender", ">=0.3.0"),
            ("testaruda", ">=0.2.0"),
            ("fotos-mcp", ">=0.3.0"),
            ("fabbro", ">=0.0.0"),
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
            std::fs::create_dir_all(parent).map_err(|e| DdlError::Io(e))?;
        }
        let contents = serde_json::to_string_pretty(self).map_err(|e| DdlError::Serde(e))?;
        std::fs::write(path, contents).map_err(|e| DdlError::Io(e))?;
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
pub fn load_compatibility(ddl_dir: &Path) -> CompatibilityMatrix {
    let cache_path = ddl_dir.join("compatibility-cache.json");

    // Try remote fetch first
    match CompatibilityMatrix::fetch() {
        Ok(matrix) => {
            // Save to cache
            let _ = matrix.save_cache(&cache_path);
            return matrix;
        }
        Err(e) => {
            eprintln!("  ⚠ Could not fetch compatibility matrix: {e}");
        }
    }

    // Try cache
    if let Some(matrix) = CompatibilityMatrix::from_cache(&cache_path) {
        return matrix;
    }

    // Fall back to embedded
    CompatibilityMatrix::embedded()
}
