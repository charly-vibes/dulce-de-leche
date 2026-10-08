//! Manifest management — read/write `.ddl/manifest.json`.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::error::{DdlError, Result};

/// The manifest file tracking installed tools and their versions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub ddl_version: String,
    pub migration_state: String,
    pub tools: HashMap<String, ToolEntry>,
    /// Tools whose init cascade ran successfully (DDL-6zn.3, idempotent-
    /// until-green): re-runs skip these instead of failing on tools that
    /// refuse re-init (e.g. `dont init` guards against overwriting state).
    #[serde(default)]
    pub inited: Vec<String>,
}

/// A single tool entry in the manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolEntry {
    pub installed: String,
    pub source: String,
    pub status: String,
    pub compatible: String,
}

impl Manifest {
    /// Create a new empty manifest.
    pub fn new() -> Self {
        Self {
            ddl_version: crate::VERSION.to_string(),
            migration_state: "none".to_string(),
            tools: HashMap::new(),
            inited: Vec::new(),
        }
    }

    /// Load the manifest from a file path.
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::new());
        }
        let contents = std::fs::read_to_string(path).map_err(DdlError::Io)?;
        Self::parse(&contents)
    }

    /// Parse a manifest from a JSON string. An empty string (freshly created
    /// file) yields a new empty manifest.
    pub fn parse(contents: &str) -> Result<Self> {
        if contents.trim().is_empty() {
            return Ok(Self::new());
        }
        let manifest: Manifest = serde_json::from_str(contents).map_err(DdlError::Serde)?;
        Ok(manifest)
    }

    /// Save the manifest to a file path.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(DdlError::Io)?;
        }
        let contents = serde_json::to_string_pretty(self).map_err(DdlError::Serde)?;
        std::fs::write(path, contents).map_err(DdlError::Io)?;
        Ok(())
    }

    /// Add or update a tool entry.
    pub fn set_tool(&mut self, name: &str, entry: ToolEntry) {
        self.tools.insert(name.to_string(), entry);
    }

    /// Get a tool entry by name.
    pub fn get_tool(&self, name: &str) -> Option<&ToolEntry> {
        self.tools.get(name)
    }

    /// Check if a tool's init cascade already ran successfully (DDL-6zn.3).
    pub fn is_inited(&self, name: &str) -> bool {
        self.inited.iter().any(|t| t == name)
    }

    /// Check if a tool is installed (status == "installed").
    pub fn is_installed(&self, name: &str) -> bool {
        self.tools
            .get(name)
            .is_some_and(|t| t.status == "installed")
    }
}

impl Default for Manifest {
    fn default() -> Self {
        Self::new()
    }
}

/// The compatibility matrix embedded in the binary.
pub const EMBEDDED_COMPATIBILITY: &str = r#"{
    "wai": ">=2026.3.0",
    "dont": ">=0.2.0",
    "ah": ">=0.2.0",
    "pretender": ">=0.3.0",
    "testaruda": ">=0.2.0",
    "vampiro": ">=0.4.0",
    "incitaciones": ">=0.8.0",
    "turu": ">=0.3.0",
    "specodelic": ">=0.1.0",
    "bd": ">=0.0.0",
    "openspec": ">=0.0.0"
}"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_manifest_json_without_inited_field_parses_with_empty_default() {
        // 0.7.0-era manifest (DDL-6zn.3 added `inited`) — must stay loadable.
        let old = r#"{
            "ddl_version": "0.7.0",
            "migration_state": "none",
            "tools": {
                "wai": {"installed": "2026.10.3", "source": "binary download", "status": "installed", "compatible": ">=2026.3.0"}
            }
        }"#;
        let m = Manifest::parse(old).unwrap();
        assert!(m.is_installed("wai"));
        assert!(!m.is_inited("wai"));
        assert!(m.inited.is_empty());
    }
}
