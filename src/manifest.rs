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
        }
    }

    /// Load the manifest from a file path.
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::new());
        }
        let contents = std::fs::read_to_string(path)
            .map_err(|e| DdlError::Io(e))?;
        let manifest: Manifest = serde_json::from_str(&contents)
            .map_err(|e| DdlError::Serde(e))?;
        Ok(manifest)
    }

    /// Save the manifest to a file path.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| DdlError::Io(e))?;
        }
        let contents = serde_json::to_string_pretty(self)
            .map_err(|e| DdlError::Serde(e))?;
        std::fs::write(path, contents)
            .map_err(|e| DdlError::Io(e))?;
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

    /// Check if a tool is installed (status == "installed").
    pub fn is_installed(&self, name: &str) -> bool {
        self.tools.get(name).map_or(false, |t| t.status == "installed")
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
    "fotos-mcp": ">=0.3.0",
    "fabbro": ">=0.0.0"
}"#;