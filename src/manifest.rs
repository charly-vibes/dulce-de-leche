//! Manifest management — read/write `.ddl/manifest.json`.
//!
//! Purpose: the typed model of the manifest file plus its parse/save
//! primitives.
//! Responsibilities: serde model, parse semantics, in-memory mutations.
//! Rationale (SEC-01, DDL-8u8): parse semantics are load-bearing for
//! durability — an empty file parses as a fresh manifest (compat with the
//! fresh-creation window), and corrupt JSON returns an error rather than
//! silently resetting established state. Interrupted writes can no longer
//! *produce* an empty or partial file: all on-disk manifest replacement is
//! staged + fsynced + renamed atomically (see `dot_ddl::write_manifest_atomic`
//! and `Manifest::save`), never truncated in place.

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

    /// Save the manifest to a file path as an atomic replace (SEC-01,
    /// DDL-8u8).
    ///
    /// Rationale: the previous implementation wrote with `std::fs::write`
    /// (truncate-then-write). Any crash mid-write would leave an empty or
    /// partial file — and since an empty file parses as a fresh manifest,
    /// established tool state was silently reset. Staging to a temp
    /// sibling, fsyncing, and renaming publishes either the complete old
    /// or the complete new file. Temp files are cleaned up on failure.
    /// Note: manifest writes that require locking/concurrency guarantees
    /// go through `DdlDir::save_manifest`/`record_tool`, which add the
    /// dedicated lock file on top of this same staged-replace pattern.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(DdlError::Io)?;
        }
        let contents = serde_json::to_string_pretty(self).map_err(DdlError::Serde)?;
        let tmp_path = path.with_extension("json.tmp");
        {
            use std::io::Write;
            let mut file = std::fs::File::create(&tmp_path).map_err(DdlError::Io)?;
            file.write_all(contents.as_bytes()).map_err(DdlError::Io)?;
            file.sync_all().map_err(DdlError::Io)?;
        }
        if let Err(e) = std::fs::rename(&tmp_path, path) {
            let _ = std::fs::remove_file(&tmp_path);
            return Err(DdlError::Io(e));
        }
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
    "funkalization": ">=0.1.0",
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
