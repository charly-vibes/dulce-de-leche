//! .ddl/ directory management — creation, migration, symlinks, gitignore.
//!
//! The `.ddl/` directory is the single source of truth for the charly-vibes
//! toolset. It contains:
//!   - manifest.json — installed tool versions, migration state
//!   - config.toml — ddl's own configuration
//!   - <tool>/ — per-tool config directories (symlinks in Phase 1)

use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use fs2::FileExt;

use crate::error::{DdlError, Result};
use crate::manifest::{Manifest, ToolEntry};

/// ddl's own configuration, written to `.ddl/config.toml`.
///
/// Implements `genesis::config::ConfigFile` for standard read/write/validate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DdlConfig {
    /// Installation source preference: "auto", "binary", "cargo", "brew", "scoop"
    #[serde(default = "default_install_source")]
    pub install_source: String,
    /// Automatically upgrade tools when running ddl init
    #[serde(default)]
    pub auto_upgrade: bool,
    /// Strategy for .gitignore entries: "auto", "prompt", "skip"
    #[serde(default = "default_gitignore_strategy")]
    pub gitignore_strategy: String,
}

fn default_install_source() -> String {
    "auto".to_string()
}

fn default_gitignore_strategy() -> String {
    "prompt".to_string()
}

impl Default for DdlConfig {
    fn default() -> Self {
        Self {
            install_source: "auto".to_string(),
            auto_upgrade: false,
            gitignore_strategy: "prompt".to_string(),
        }
    }
}

impl genesis::config::ConfigFile for DdlConfig {
    /// The config lives at `<ddl_dir>/config.toml`.
    ///
    /// Although the genesis `ConfigFile` trait calls this `repo_root`,
    /// ddl passes its `.ddl/` directory path here (not the project root).
    fn path(repo_root: &Path) -> PathBuf {
        repo_root.join("config.toml")
    }

    fn validate(
        &self,
    ) -> std::result::Result<Vec<genesis::config::ConfigValidation>, genesis::config::ConfigError>
    {
        let mut results = Vec::new();
        match self.install_source.as_str() {
            "auto" | "binary" | "cargo" | "brew" | "scoop" => {}
            other => results.push(genesis::config::ConfigValidation::warning(
                "install_source",
                format!("unknown install source '{}', expected one of: auto, binary, cargo, brew, scoop", other),
            )),
        }
        match self.gitignore_strategy.as_str() {
            "auto" | "prompt" | "skip" => {}
            other => results.push(genesis::config::ConfigValidation::warning(
                "gitignore_strategy",
                format!(
                    "unknown gitignore strategy '{}', expected one of: auto, prompt, skip",
                    other
                ),
            )),
        }
        Ok(results)
    }
}

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
    // Note: `fotos-mcp` is intentionally excluded — it has no known legacy
    // config file/directory to migrate; its MCP server config lives elsewhere.
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
            std::fs::create_dir_all(&tool_dir).map_err(DdlError::Io)?;
        }
        Ok(tool_dir)
    }

    /// Save the manifest to disk with an exclusive file lock.
    ///
    /// The lock prevents concurrent manifest writes from different processes
    /// from losing entries. Lock is released when the opened file is dropped.
    pub fn save_manifest(&self) -> Result<()> {
        let manifest_path = self.manifest_path();
        // Open for read+write+create (no truncate) to get a handle for locking.
        // The actual write happens via Manifest::save which opens separately,
        // but the advisory lock on the inode prevents concurrent writers.
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&manifest_path)
            .map_err(DdlError::Io)?;
        file.lock_exclusive().map_err(DdlError::Io)?;
        self.manifest.save(&manifest_path)
    }

    /// Update a tool entry in the manifest and save atomically.
    ///
    /// Uses an exclusive file lock to prevent concurrent manifest writes
    /// from different processes from losing entries. Re-reads the manifest
    /// from disk inside the lock to get the latest state.
    pub fn record_tool(&mut self, name: &str, entry: ToolEntry) -> Result<()> {
        let manifest_path = self.manifest_path();

        // Open for read+write+create (no truncate) to get a handle for locking.
        // Reading the file before truncating preserves existing content.
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&manifest_path)
            .map_err(DdlError::Io)?;
        file.lock_exclusive().map_err(DdlError::Io)?;

        // Re-read manifest from disk inside the lock to get the latest
        // state (catches any concurrent modifications).
        let mut manifest = Manifest::load(&manifest_path)?;
        manifest.set_tool(name, entry);
        manifest.save(&manifest_path)?;

        // Update in-memory state to match disk
        self.manifest = manifest;

        // File is dropped, releasing the lock
        Ok(())
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
                let mut file =
                    std::fs::File::create(&gitignore_path).map_err(DdlError::Io)?;
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

    /// Migrate a single tool's config from legacy location to .ddl/.
    /// Phase 1: move contents to .ddl/<tool>/, create symlink at legacy location.
    pub fn migrate_tool(&self, tool_name: &str, legacy_path: &Path) -> Result<()> {
        let ddl_tool_path = self.tool_path(tool_name);

        // Create target directory
        if !ddl_tool_path.exists() {
            std::fs::create_dir_all(&ddl_tool_path).map_err(DdlError::Io)?;
        }

        // Idempotency guard: if legacy path is already a symlink, it was
        // migrated in a previous run. Skip to avoid data corruption.
        if legacy_path.is_symlink() {
            return Ok(());
        }

        // Track whether the original was a file, for correct symlink and undo.
        let legacy_path_was_file = legacy_path.is_file();

        // Move contents from legacy to .ddl/<tool>/
        if legacy_path.is_dir() {
            for entry in std::fs::read_dir(legacy_path).map_err(DdlError::Io)? {
                let entry = entry.map_err(DdlError::Io)?;
                let target = ddl_tool_path.join(entry.file_name());
                std::fs::rename(entry.path(), &target).map_err(DdlError::Io)?;
            }
            // Remove empty legacy directory
            std::fs::remove_dir(legacy_path).map_err(DdlError::Io)?;
        } else if legacy_path_was_file {
            // Single file config (e.g., .pretender.toml)
            let target = ddl_tool_path.join(legacy_path.file_name().unwrap_or_default());
            std::fs::rename(legacy_path, &target).map_err(DdlError::Io)?;
        }

        // Create symlink at legacy location.
        // For single-file configs, point to the file inside .ddl/<tool>/, not the directory.
        let symlink_target = if legacy_path_was_file {
            ddl_tool_path.join(legacy_path.file_name().unwrap_or_default())
        } else {
            ddl_tool_path.clone()
        };
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&symlink_target, legacy_path).map_err(DdlError::Io)?;
        }
        #[cfg(windows)]
        {
            if symlink_target.is_dir() {
                std::os::windows::fs::symlink_dir(&symlink_target, legacy_path)
                    .map_err(|e| DdlError::Io(e))?;
            } else {
                std::os::windows::fs::symlink_file(&symlink_target, legacy_path)
                    .map_err(|e| DdlError::Io(e))?;
            }
        }

        Ok(())
    }

    /// Undo migration for a single tool: remove symlink, move contents back.
    ///
    /// Detects single-file configs (e.g. `.pretender.toml`) by checking the
    /// symlink target type before removing it, so the correct restore method
    /// (file vs directory) is used.
    pub fn unmigrate_tool(&self, tool_name: &str, legacy_path: &Path) -> Result<()> {
        let ddl_tool_path = self.tool_path(tool_name);

        // Check symlink target type BEFORE removing it — this tells us whether
        // the original was a single-file config (symlink points to a file)
        // or a directory config (symlink points to a directory).
        let is_single_file = std::fs::read_link(legacy_path)
            .ok()
            .map(|target| {
                let resolved = if target.is_relative() {
                    legacy_path.parent().unwrap_or(Path::new(".")).join(&target)
                } else {
                    target
                };
                resolved.is_file()
            })
            .unwrap_or(false);

        // Remove symlink
        if legacy_path.exists() {
            std::fs::remove_file(legacy_path)
                .or_else(|_| std::fs::remove_dir(legacy_path))
                .map_err(DdlError::Io)?;
        }

        // Move contents back
        if ddl_tool_path.exists() {
            if is_single_file {
                // Single-file config: move the file directly to legacy_path
                let entries: Vec<_> = std::fs::read_dir(&ddl_tool_path)
                    .map_err(DdlError::Io)?
                    .filter_map(|e| e.ok())
                    .collect();
                if let Some(entry) = entries.into_iter().next() {
                    std::fs::rename(entry.path(), legacy_path).map_err(DdlError::Io)?;
                }
            } else {
                // Directory config: create legacy directory, move files back
                std::fs::create_dir_all(legacy_path).map_err(DdlError::Io)?;
                for entry in std::fs::read_dir(&ddl_tool_path).map_err(DdlError::Io)? {
                    let entry = entry.map_err(DdlError::Io)?;
                    let target = legacy_path.join(entry.file_name());
                    std::fs::rename(entry.path(), &target).map_err(DdlError::Io)?;
                }
            }
            std::fs::remove_dir(&ddl_tool_path).map_err(DdlError::Io)?;
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
                if let Ok(meta) = std::fs::symlink_metadata(&path)
                    && meta.is_symlink() {
                        // Check if the target exists
                        if std::fs::metadata(&path).is_err() {
                            broken.push(path);
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
                        .map_err(DdlError::Io)?;
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
                        messages.push(format!(
                            "✗ {} — recorded as installed but not on PATH",
                            name
                        ));
                        if fix {
                            messages.push(format!("  ℹ  Run `ddl install {}` to reinstall", name));
                        }
                    }
                }
                "failed" => {
                    messages.push(format!(
                        "✗ {} — previously failed (via {})",
                        name, entry.source
                    ));
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
///
/// Detects both directory and single-file symlink targets (e.g. `.pretender.toml`
/// pointing to `.ddl/pretender/.pretender.toml`). Resolves relative symlink targets
/// to absolute before comparing, and normalises the expected path so that detection
/// works regardless of whether the DdlDir was created with a relative or absolute path.
pub fn migrated_tools(ddl_dir: &DdlDir) -> Vec<(String, PathBuf)> {
    let mut result = Vec::new();
    for (tool_name, legacy_path_str) in LEGACY_CONFIGS {
        let legacy_path = PathBuf::from(legacy_path_str);
        if is_symlink(&legacy_path)
            && let Ok(target) = std::fs::read_link(&legacy_path)
        {
                // Resolve relative symlink targets to absolute (relative to the
                // symlink's parent directory) before comparing.
                let resolved = if target.is_relative() {
                    legacy_path.parent().unwrap_or(Path::new(".")).join(&target)
                } else {
                    target.clone()
                };
                // Canonicalise both resolved and expected paths so that relative
                // vs absolute DdlDir paths don't break detection.
                let resolved = std::fs::canonicalize(&resolved).unwrap_or(resolved);
                let expected = ddl_dir.tool_path(tool_name);
                let expected = std::fs::canonicalize(&expected).unwrap_or(expected);
                // Directory configs: symlink points directly to .ddl/<tool>/.
                // Single-file configs: symlink points to a file inside .ddl/<tool>/.
                if resolved == expected || resolved.parent() == Some(&expected) {
                    result.push((tool_name.to_string(), legacy_path));
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
            {
                m.is_symlink()
            }
            #[cfg(windows)]
            {
                m.file_type().is_symlink()
            }
        })
        .unwrap_or(false)
}
