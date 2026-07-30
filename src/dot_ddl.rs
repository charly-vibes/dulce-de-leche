//! .ddl/ directory management — creation, migration, symlinks, gitignore.
//!
//! The `.ddl/` directory is the single source of truth for the charly-vibes
//! toolset. It contains:
//!   - manifest.json — installed tool versions, migration state
//!   - config.toml — ddl's own configuration
//!   - <tool>/ — per-tool config directories (symlinks in Phase 1)

use std::io::Write;
use std::path::{Path, PathBuf};

use crate::error::{DdlError, Result};
use crate::manifest::{Manifest, ToolEntry};

/// Default ddl configuration.
const DEFAULT_CONFIG: &str = r##"# dulce-de-leche configuration
# See https://github.com/charly-vibes/dulce-de-leche for documentation

# Installation source preference: "auto", "binary", "cargo", "brew", "scoop"
install_source = "auto"

# Automatically upgrade tools when running ddl init
auto_upgrade = false

# Strategy for .gitignore entries: "auto", "prompt", "skip"
gitignore_strategy = "prompt"
"##;

/// The .gitignore entries to add for .ddl/ data files.
pub const GITIGNORE_ENTRIES: &str = r##"
# dulce-de-leche — data files (do not commit)
.ddl/**/*.db
.ddl/**/store/
.ddl/install-log.json
.ddl/doctor-cache.json
.ddl/compatibility-cache.json
"##;

/// Legacy tool config directories mapped to their binary names.
/// Used by `ddl migrate` to detect existing configs.
const LEGACY_CONFIGS: &[(&str, &str)] = &[
    ("wai", ".wai"),
    ("dont", ".dont"),
    ("ah", ".espectacular"),
    ("pretender", ".pretender.toml"),
    ("testaruda", ".testaruda"),
    ("fabbro", ".fabbro"),
];

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

    /// Create a new `.ddl/` directory at the given path.
    pub fn create_at(path: &Path) -> Result<Self> {
        if path.exists() {
            let manifest = Manifest::load(&path.join("manifest.json"))?;
            return Ok(Self {
                path: path.to_path_buf(),
                manifest,
            });
        }

        std::fs::create_dir_all(path)
            .map_err(|e| DdlError::Io(e))?;

        // Create config.toml
        let config_path = path.join("config.toml");
        if !config_path.exists() {
            let mut file = std::fs::File::create(&config_path)
                .map_err(|e| DdlError::Io(e))?;
            file.write_all(DEFAULT_CONFIG.as_bytes())
                .map_err(|e| DdlError::Io(e))?;
        }

        let manifest = Manifest::new();
        let manifest_path = path.join("manifest.json");
        manifest.save(&manifest_path)?;

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
            std::fs::create_dir_all(&tool_dir)
                .map_err(|e| DdlError::Io(e))?;
        }
        Ok(tool_dir)
    }

    /// Save the manifest to disk.
    pub fn save_manifest(&self) -> Result<()> {
        self.manifest.save(&self.manifest_path())
    }

    /// Update a tool entry in the manifest and save.
    pub fn record_tool(&mut self, name: &str, entry: ToolEntry) -> Result<()> {
        self.manifest.set_tool(name, entry);
        self.save_manifest()
    }

    /// Record a tool as installed, with version and source.
    pub fn record_installed(
        &mut self,
        name: &str,
        version: &str,
        source: &str,
    ) -> Result<()> {
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
                let mut file = std::fs::File::create(&gitignore_path)
                    .map_err(|e| DdlError::Io(e))?;
                file.write_all(GITIGNORE_ENTRIES.as_bytes())
                    .map_err(|e| DdlError::Io(e))?;
                println!("  ✓ .gitignore created");
            }
            return Ok(());
        }

        let contents = std::fs::read_to_string(&gitignore_path)
            .map_err(|e| DdlError::Io(e))?;

        if contents.contains(".ddl/**/*.db") {
            return Ok(()); // already has entries
        }

        if yes {
            let mut file = std::fs::OpenOptions::new()
                .append(true)
                .open(&gitignore_path)
                .map_err(|e| DdlError::Io(e))?;
            file.write_all(GITIGNORE_ENTRIES.as_bytes())
                .map_err(|e| DdlError::Io(e))?;
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

    /// Migrate a single tool's config from legacy location to .ddl/.
    /// Phase 1: move contents to .ddl/<tool>/, create symlink at legacy location.
    pub fn migrate_tool(&self, tool_name: &str, legacy_path: &Path) -> Result<()> {
        let ddl_tool_path = self.tool_path(tool_name);

        // Create target directory
        if !ddl_tool_path.exists() {
            std::fs::create_dir_all(&ddl_tool_path)
                .map_err(|e| DdlError::Io(e))?;
        }

        // Move contents from legacy to .ddl/<tool>/
        if legacy_path.is_dir() {
            for entry in std::fs::read_dir(legacy_path)
                .map_err(|e| DdlError::Io(e))?
            {
                let entry = entry.map_err(|e| DdlError::Io(e))?;
                let target = ddl_tool_path.join(entry.file_name());
                std::fs::rename(&entry.path(), &target)
                    .map_err(|e| DdlError::Io(e))?;
            }
            // Remove empty legacy directory
            std::fs::remove_dir(legacy_path)
                .map_err(|e| DdlError::Io(e))?;
        } else if legacy_path.is_file() {
            // Single file config (e.g., .pretender.toml)
            let target = ddl_tool_path.join(
                legacy_path.file_name().unwrap_or_default(),
            );
            std::fs::rename(legacy_path, &target)
                .map_err(|e| DdlError::Io(e))?;
        }

        // Create symlink at legacy location
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&ddl_tool_path, legacy_path)
                .map_err(|e| DdlError::Io(e))?;
        }
        #[cfg(windows)]
        {
            if legacy_path.is_dir() || ddl_tool_path.is_dir() {
                std::os::windows::fs::symlink_dir(&ddl_tool_path, legacy_path)
                    .map_err(|e| DdlError::Io(e))?;
            } else {
                std::os::windows::fs::symlink_file(&ddl_tool_path, legacy_path)
                    .map_err(|e| DdlError::Io(e))?;
            }
        }

        Ok(())
    }

    /// Undo migration for a single tool: remove symlink, move contents back.
    pub fn unmigrate_tool(&self, tool_name: &str, legacy_path: &Path) -> Result<()> {
        let ddl_tool_path = self.tool_path(tool_name);

        // Remove symlink
        if legacy_path.exists() {
            std::fs::remove_file(legacy_path)
                .or_else(|_| std::fs::remove_dir(legacy_path))
                .map_err(|e| DdlError::Io(e))?;
        }

        // Move contents back
        if ddl_tool_path.exists() {
            std::fs::create_dir_all(legacy_path)
                .map_err(|e| DdlError::Io(e))?;
            for entry in std::fs::read_dir(&ddl_tool_path)
                .map_err(|e| DdlError::Io(e))?
            {
                let entry = entry.map_err(|e| DdlError::Io(e))?;
                let target = legacy_path.join(entry.file_name());
                std::fs::rename(&entry.path(), &target)
                    .map_err(|e| DdlError::Io(e))?;
            }
            std::fs::remove_dir(&ddl_tool_path)
                .map_err(|e| DdlError::Io(e))?;
        }

        Ok(())
    }

    /// Detect broken symlinks in the .ddl/ directory.
    pub fn detect_broken_symlinks(&self) -> Vec<PathBuf> {
        let mut broken = Vec::new();
        if !self.path.exists() {
            return broken;
        }

        let entries = match std::fs::read_dir(&self.path) {
            Ok(e) => e,
            Err(_) => return broken,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            // Check if it's a symlink
            #[cfg(unix)]
            {
                if let Ok(meta) = std::fs::symlink_metadata(&path) {
                    if meta.is_symlink() {
                        // Check if the target exists
                        if std::fs::metadata(&path).is_err() {
                            broken.push(path);
                        }
                    }
                }
            }
            #[cfg(windows)]
            {
                if let Ok(meta) = std::fs::symlink_metadata(&path) {
                    if meta.file_type().is_symlink() {
                        if std::fs::metadata(&path).is_err() {
                            broken.push(path);
                        }
                    }
                }
            }
        }

        broken
    }

    /// Run doctor checks on the .ddl/ directory.
    pub fn doctor(&self, fix: bool) -> Result<Vec<String>> {
        let mut messages = Vec::new();

        // Check .ddl/ exists
        if !self.path.exists() {
            messages.push("✗ .ddl/ directory not found".to_string());
            if fix {
                Self::create_at(&self.path)?;
                messages.push("  ✓ .ddl/ created".to_string());
            }
            return Ok(messages);
        }
        messages.push("✓ .ddl/ directory found".to_string());

        // Check manifest
        let manifest_path = self.manifest_path();
        if manifest_path.exists() {
            messages.push(format!(
                "✓ manifest.json ({} tools tracked)",
                self.manifest.tools.len()
            ));
        } else {
            messages.push("✗ manifest.json not found".to_string());
            if fix {
                self.save_manifest()?;
                messages.push("  ✓ manifest.json created".to_string());
            }
        }

        // Check config
        let config_path = self.config_path();
        if config_path.exists() {
            messages.push("✓ config.toml found".to_string());
        } else {
            messages.push("○ config.toml not found (will be created on next init)".to_string());
        }

        // Check broken symlinks
        let broken = self.detect_broken_symlinks();
        if broken.is_empty() {
            messages.push("✓ no broken symlinks".to_string());
        } else {
            for symlink in &broken {
                messages.push(format!("✗ broken symlink: {}", symlink.display()));
                if fix {
                    std::fs::remove_file(symlink)
                        .or_else(|_| std::fs::remove_dir(symlink))
                        .map_err(|e| DdlError::Io(e))?;
                    messages.push(format!("  ✓ removed broken symlink: {}", symlink.display()));
                }
            }
        }

        // Check per-tool status
        for (name, entry) in &self.manifest.tools {
            match entry.status.as_str() {
                "installed" => {
                    if crate::installer::is_tool_installed(name) {
                        messages.push(format!("✓ {} — installed and on PATH", name));
                    } else {
                        messages.push(format!("✗ {} — recorded as installed but not on PATH", name));
                        if fix {
                            messages.push(format!("  ℹ  Run `ddl install {}` to reinstall", name));
                        }
                    }
                }
                "failed" => {
                    messages.push(format!("✗ {} — previously failed (via {})", name, entry.source));
                    if fix {
                        messages.push(format!("  ℹ  Run `ddl install {}` to retry", name));
                    }
                }
                _ => {
                    messages.push(format!("○ {} — unknown status: {}", name, entry.status));
                }
            }
        }

        Ok(messages)
    }
}

/// The status of a tool as recorded in the manifest.
#[derive(Debug, Clone)]
pub enum ToolStatus {
    Installed { version: String, source: String },
    Failed { source: String },
    NotTracked,
    Unknown,
}

/// Collect all tool config directories that have been migrated.
/// Returns a map of tool name to legacy path.
pub fn migrated_tools(ddl_dir: &DdlDir) -> Vec<(String, PathBuf)> {
    let mut result = Vec::new();
    for (tool_name, legacy_path_str) in LEGACY_CONFIGS {
        let legacy_path = PathBuf::from(legacy_path_str);
        if is_symlink(&legacy_path) {
            // Check if it points to .ddl/<tool>/
            if let Ok(target) = std::fs::read_link(&legacy_path) {
                let expected = ddl_dir.tool_path(tool_name);
                if target == expected {
                    result.push((tool_name.to_string(), legacy_path));
                }
            }
        }
    }
    result
}

/// Check if a path is a symlink.
fn is_symlink(path: &Path) -> bool {
    std::fs::symlink_metadata(path)
        .map(|m| {
            #[cfg(unix)]
            { m.is_symlink() }
            #[cfg(windows)]
            { m.file_type().is_symlink() }
        })
        .unwrap_or(false)
}