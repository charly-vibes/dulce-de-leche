//! DdlDir: discovery, creation, manifest recording, tool status, gitignore wiring.

use super::persist::{manifest_lock_guard, with_sharing_retry, write_manifest_atomic};
use super::*;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::error::{DdlError, Result};
use crate::manifest::{Manifest, ToolEntry};

/// Manage the `.ddl/` directory.
#[derive(Debug, Clone)]
pub struct DdlDir {
    pub path: PathBuf,
    pub manifest: Manifest,
}

impl DdlDir {
    /// Find the nearest `.ddl/` directory by walking up from CWD.
    /// If not found, create one at CWD.
    pub fn find_or_create() -> Result<Self> {
        match crate::find_ddl_dir() {
            Some(path) => {
                let manifest = Manifest::load(&path.join("manifest.json"))?;
                Ok(Self { path, manifest })
            }
            None => {
                let path = PathBuf::from(".ddl");
                Self::create_at(&path)
            }
        }
    }

    /// Find the nearest `.ddl/` directory by walking up from CWD (read-only).
    /// Returns `None` if no `.ddl/` directory exists, without creating one.
    pub fn find() -> Option<Self> {
        let path = crate::find_ddl_dir()?;
        let manifest = Manifest::load(&path.join("manifest.json")).ok()?;
        Some(Self { path, manifest })
    }

    /// Create a new `.ddl/` directory at the given path.
    pub fn create_at(path: &Path) -> Result<Self> {
        if path.exists() {
            let manifest = Manifest::load(&path.join("manifest.json"))?;
            return Ok(Self {
                path: path.to_path_buf(),
                manifest,
            });
        }

        std::fs::create_dir_all(path).map_err(DdlError::Io)?;

        // Create config.toml via genesis ConfigFile (preserve existing)
        let config_path = path.join("config.toml");
        if !config_path.exists() {
            let config = DdlConfig::default();
            genesis::config::ConfigFile::write(&config, path)
                .map_err(|e| DdlError::Other(e.to_string()))?;
        }

        let manifest = Manifest::new();
        let manifest_path = path.join("manifest.json");
        ensure_manifest_write_is_local(&manifest_path)?;
        // Atomic replace (SEC-01, DDL-8u8) through a no-follow staged
        // file (SEC-05): std::fs::write would follow a link swapped in
        // after the guard check and truncates in place.
        write_manifest_atomic(&manifest_path, &manifest)?;

        Ok(Self {
            path: path.to_path_buf(),
            manifest,
        })
    }

    /// Path to the manifest file.
    pub fn manifest_path(&self) -> PathBuf {
        self.path.join("manifest.json")
    }

    /// Path to the config file.
    pub fn config_path(&self) -> PathBuf {
        self.path.join("config.toml")
    }

    /// Path to a tool's config directory/symlink under .ddl/.
    pub fn tool_path(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }

    /// Create a tool's config directory under .ddl/.
    pub fn ensure_tool_dir(&self, name: &str) -> Result<PathBuf> {
        let tool_dir = self.tool_path(name);
        if !tool_dir.exists() {
            std::fs::create_dir_all(&tool_dir).map_err(DdlError::Io)?;
        }
        Ok(tool_dir)
    }

    /// Save the manifest to disk as an atomic replace under an exclusive
    /// lock (SEC-01, DDL-8u8).
    ///
    /// Serialization uses the dedicated [`manifest_lock_guard`] lock file
    /// (stable, never renamed) so the lock can be held through the rename;
    /// the replacement itself is staged + fsynced + renamed (see
    /// [`write_manifest_atomic`]). Lock is released when the lock handle is
    /// dropped. The with_sharing_retry wrapper handles genuine transient
    /// Windows locks (e.g. antivirus scanning) on the open/lock/rename
    /// steps (DDL-71i).
    pub fn save_manifest(&self) -> Result<()> {
        let manifest_path = self.manifest_path();
        with_sharing_retry(|| self.try_save_manifest(&manifest_path))
    }

    fn try_save_manifest(&self, manifest_path: &Path) -> Result<()> {
        ensure_manifest_write_is_local(manifest_path)?;
        let _lock = manifest_lock_guard(manifest_path)?;
        write_manifest_atomic(manifest_path, &self.manifest)
    }

    /// Update a tool entry in the manifest and save atomically.
    ///
    /// Uses the dedicated lock file to prevent concurrent manifest writes
    /// from different processes from losing entries (SEC-01, DDL-8u8).
    /// Re-reads the manifest from disk inside the lock to get the latest
    /// state, then installs it with an atomic staged replace — readers
    /// observe complete-old or complete-new only.
    pub fn record_tool(&mut self, name: &str, entry: ToolEntry) -> Result<()> {
        let manifest_path = self.manifest_path();
        with_sharing_retry(|| {
            ensure_manifest_write_is_local(&manifest_path)?;
            // Read-modify-write serialized by the dedicated lock file, held
            // through the atomic rename (see save_manifest). The lock file
            // open refuses link redirection (SEC-05).
            let _lock = manifest_lock_guard(&manifest_path)?;

            // Re-read manifest from disk inside the lock to get the latest
            // state (catches any concurrent modifications). With atomic
            // replacement, the file is always a complete manifest.
            let mut manifest = Manifest::load(&manifest_path)?;
            manifest.set_tool(name, entry.clone());
            write_manifest_atomic(&manifest_path, &manifest)?;

            // Update in-memory state to match disk
            self.manifest = manifest;

            Ok(())
        })
    }

    /// Record a tool as installed, with version and source.
    pub fn record_installed(&mut self, name: &str, version: &str, source: &str) -> Result<()> {
        self.record_tool(
            name,
            ToolEntry {
                installed: version.to_string(),
                source: source.to_string(),
                status: "installed".to_string(),
                compatible: ">=0.0.0".to_string(),
            },
        )
    }

    /// Record that a tool's init cascade ran successfully (DDL-6zn.3).
    ///
    /// Idempotent-until-green: re-runs of `ddl init` skip tools listed here
    /// instead of failing on tools that refuse re-init (e.g. `dont init`
    /// errors when project state already exists).
    pub fn record_inited(&mut self, name: &str) -> Result<()> {
        if !self.manifest.is_inited(name) {
            self.manifest.inited.push(name.to_string());
            self.save_manifest()?;
        }
        Ok(())
    }

    /// Record a tool as failed.
    pub fn record_failed(&mut self, name: &str, source: &str) -> Result<()> {
        self.record_tool(
            name,
            ToolEntry {
                installed: "unknown".to_string(),
                source: source.to_string(),
                status: "failed".to_string(),
                compatible: ">=0.0.0".to_string(),
            },
        )
    }

    /// Get the status of a tool from the manifest.
    pub fn tool_status(&self, name: &str) -> ToolStatus {
        match self.manifest.get_tool(name) {
            Some(entry) if entry.status == "installed" => ToolStatus::Installed {
                version: entry.installed.clone(),
                source: entry.source.clone(),
            },
            Some(entry) if entry.status == "failed" => ToolStatus::Failed {
                source: entry.source.clone(),
            },
            Some(_) => ToolStatus::Unknown,
            None => ToolStatus::NotTracked,
        }
    }

    /// Check if a tool has a failed installation and should be retried.
    pub fn should_retry(&self, name: &str) -> bool {
        matches!(self.tool_status(name), ToolStatus::Failed { .. })
    }

    /// Get all tools that failed and need retry.
    pub fn failed_tools(&self) -> Vec<String> {
        self.manifest
            .tools
            .iter()
            .filter(|(_, e)| e.status == "failed")
            .map(|(name, _)| name.clone())
            .collect()
    }

    /// Add .gitignore entries for .ddl/ data files.
    pub fn add_gitignore_entries(&self, yes: bool) -> Result<()> {
        let gitignore_path = PathBuf::from(".gitignore");
        if !gitignore_path.exists() {
            if yes {
                let mut file = std::fs::File::create(&gitignore_path).map_err(DdlError::Io)?;
                file.write_all(GITIGNORE_ENTRIES.as_bytes())
                    .map_err(DdlError::Io)?;
                println!("  ✓ .gitignore created");
            }
            return Ok(());
        }

        let contents = std::fs::read_to_string(&gitignore_path).map_err(DdlError::Io)?;

        if contents.contains(".ddl/**/*.db") {
            return Ok(()); // already has entries
        }

        if yes {
            let mut file = std::fs::OpenOptions::new()
                .append(true)
                .open(&gitignore_path)
                .map_err(DdlError::Io)?;
            file.write_all(GITIGNORE_ENTRIES.as_bytes())
                .map_err(DdlError::Io)?;
            println!("  ✓ .gitignore updated");
        } else {
            println!("  ℹ  Run `ddl init --yes` to auto-add .gitignore entries");
        }

        Ok(())
    }

    // ===== Phase 1 Migration (Symlink Farm) =====

    /// Detect legacy config directories that can be migrated.
    pub fn detect_legacy_configs(&self) -> Vec<(&'static str, PathBuf)> {
        let mut found = Vec::new();
        for &(tool_name, legacy_path_str) in LEGACY_CONFIGS {
            let legacy_path = PathBuf::from(legacy_path_str);
            if legacy_path.exists() {
                found.push((tool_name, legacy_path));
            }
        }
        found
    }
}
