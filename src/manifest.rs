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
//!
//! Rationale (DDL-5bs, SEC-04): the manifest stays the *install inventory*
//! (what is installed); artifact identities live in a separate install lock
//! (`InstallLock`, `.ddl/lock.json`) so inventory bookkeeping can never
//! corrupt a recorded version/URL/digest identity, and a locked replay can
//! reproduce the exact verified bytes.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::error::{DdlError, Result};

/// The fleet-global version pins file, embedded from ddl's own
/// `versions.ddl.toml` at build time (DDL-fw7).
///
/// Rationale: docs/standardization.md §pins makes ddl's own pins the
/// canonical fleet source — per-repo CI installs are pinned to the same
/// versions ddl itself dogfoods. `ddl init` scaffolds a repo's
/// `versions.ddl.toml` from this constant when the repo has none; the
/// repo then owns (and deliberately updates) its copy.
pub const FLEET_VERSIONS_PIN: &str = include_str!("../versions.ddl.toml");

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

/// A recorded artifact identity for one tool installation (DDL-5bs, SEC-04).
///
/// This is what a locked replay reproduces: the exact version, the concrete
/// (tag-pinned) artifact URL the verified bytes came from, the sha256 digest
/// those bytes matched under the DDL-yts/wof policies, and the day the
/// identity was recorded (a deterministic time input for the release-age
/// policy — never the wall clock).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LockEntry {
    pub tool: String,
    pub version: String,
    pub artifact_url: String,
    pub sha256: String,
    pub recorded_epoch_day: u64,
}

/// The install lock: `.ddl/lock.json`, the identity record of what was
/// verified (DDL-5bs, SEC-04).
///
/// Distinct from the [`Manifest`] inventory by design: the lock can be
/// replayed, shared and audited without touching inventory state, and
/// inventory mutations cannot corrupt an artifact identity.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallLock {
    #[serde(default)]
    pub entries: HashMap<String, LockEntry>,
}

impl InstallLock {
    /// Create a new empty lock.
    pub fn new() -> Self {
        Self::default()
    }

    /// Load the lock from a file path. A missing file is an empty lock —
    /// the only migration state (v1 is the first lock schema; tools replay
    /// nothing until the next verified install records an identity).
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::new());
        }
        let contents = std::fs::read_to_string(path).map_err(DdlError::Io)?;
        Self::parse(&contents)
    }

    /// Parse a lock from a JSON string. An empty string (freshly created
    /// file) yields an empty lock, matching the manifest's fresh-creation
    /// window semantics.
    pub fn parse(contents: &str) -> Result<Self> {
        if contents.trim().is_empty() {
            return Ok(Self::new());
        }
        let lock: InstallLock = serde_json::from_str(contents).map_err(DdlError::Serde)?;
        Ok(lock)
    }

    /// Save the lock as an atomic staged replace (same pattern as
    /// [`Manifest::save`], DDL-8u8): a crash leaves either the old or the new
    /// lock, never a partial file.
    pub fn save(&self, path: &Path) -> Result<()> {
        write_file_atomic(
            path,
            &serde_json::to_string_pretty(self).map_err(DdlError::Serde)?,
        )
    }

    /// Record (or replace) a tool's artifact identity.
    pub fn record(&mut self, entry: LockEntry) {
        self.entries.insert(entry.tool.clone(), entry);
    }

    /// Get a tool's recorded identity, if any.
    pub fn get(&self, tool: &str) -> Option<&LockEntry> {
        self.entries.get(tool)
    }
}

/// Atomically replace a file with `contents`: stage to a temp sibling,
/// fsync, rename. Shared by the manifest and install-lock saves so both
/// publish either the complete old or the complete new file.
fn write_file_atomic(path: &Path, contents: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(DdlError::Io)?;
    }
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
    "openspec": ">=0.0.0",
    "wild": ">=0.0.0"
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
