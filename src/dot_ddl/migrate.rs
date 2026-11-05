//! Migration: symlink configs under .ddl/, undo, broken-symlink detection.
use super::*;
use std::path::{Path, PathBuf};

use crate::error::{DdlError, Result};

impl DdlDir {
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

    /// Undo migration for all migrated tools at once (bulk `--undo`).
    ///
    /// Non-destructive: each legacy symlink is replaced by a real copy of
    /// the current `.ddl/<tool>/` contents, but the `.ddl/` backing store is
    /// left untouched — so the undo can never lose tool state, and a
    /// subsequent `ddl migrate` re-links losslessly.
    ///
    /// Returns the `(tool, legacy_path)` pairs that were restored.
    pub fn unmigrate_all(&self) -> Result<Vec<(String, PathBuf)>> {
        let migrated = migrated_tools(self);
        for (tool_name, legacy_path) in &migrated {
            let ddl_tool_path = self.tool_path(tool_name);

            // Read the symlink target type BEFORE removing it — single-file
            // configs (symlink → a file inside .ddl/<tool>/) restore as a
            // single file; directory configs restore the whole tree.
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

            // Remove the legacy symlink (not its target).
            std::fs::remove_file(legacy_path)
                .or_else(|_| std::fs::remove_dir(legacy_path))
                .map_err(DdlError::Io)?;

            // Copy the current .ddl/<tool>/ contents back to the legacy
            // location. Copy (not move) keeps the backing store intact.
            if is_single_file {
                let file_in_tool_dir =
                    ddl_tool_path.join(legacy_path.file_name().unwrap_or_default());
                std::fs::copy(&file_in_tool_dir, legacy_path).map_err(DdlError::Io)?;
            } else if ddl_tool_path.is_dir() {
                std::fs::create_dir_all(legacy_path).map_err(DdlError::Io)?;
                for entry in std::fs::read_dir(&ddl_tool_path).map_err(DdlError::Io)? {
                    let entry = entry.map_err(DdlError::Io)?;
                    Self::copy_entry(&entry.path(), &legacy_path.join(entry.file_name()))?;
                }
            } else if ddl_tool_path.is_file() {
                std::fs::copy(&ddl_tool_path, legacy_path).map_err(DdlError::Io)?;
            }
        }
        // Return absolute paths — migrated_tools() yields CWD-relative legacy
        // paths, which are ambiguous outside the migrating process.
        Ok(migrated
            .into_iter()
            .map(|(tool, path)| {
                let abs = std::fs::canonicalize(&path).unwrap_or(path);
                (tool, abs)
            })
            .collect())
    }

    /// Recursively copy a file or directory tree.
    fn copy_entry(src: &Path, dst: &Path) -> Result<()> {
        if src.is_dir() {
            std::fs::create_dir_all(dst).map_err(DdlError::Io)?;
            for entry in std::fs::read_dir(src).map_err(DdlError::Io)? {
                let entry = entry.map_err(DdlError::Io)?;
                Self::copy_entry(&entry.path(), &dst.join(entry.file_name()))?;
            }
        } else {
            std::fs::copy(src, dst).map_err(DdlError::Io)?;
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
                    && meta.is_symlink()
                {
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

/// Best-effort check whether a path is tracked by git (staged or committed).
///
/// Delegates to `genesis::git::tracked` (`git ls-files --error-unmatch`),
/// resolving the repository root explicitly from the path's parent so the
/// invocation never relies on the current working directory. Returns false
/// when git is unavailable, the path is outside a repository, or the path is
/// untracked — in which case symlink migration is safe (git does not follow
/// symlinks, but untracked content has no history to corrupt).
pub fn is_git_tracked(path: &Path) -> bool {
    // `Path::parent` of a bare relative name (e.g. ".wai") is Some(""); the
    // previous `git -C ""` invocation treated that as a cwd no-op, so map it
    // to "." to keep the same effective directory.
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    match genesis::git::repo_root_from(parent) {
        Ok(root) => {
            // Whole-repo probe: `tracked` relativizes path == root to an
            // empty pathspec, which git rejects (exit 128). The previous
            // `ls-files --error-unmatch <abs root>` matched whenever the
            // repo knew any path, so use "." for that case.
            let probe = if path == root { Path::new(".") } else { path };
            genesis::git::tracked(&root, probe).unwrap_or(false)
        }
        // Outside any repository: best-effort degradation, same as untracked.
        Err(_) => false,
    }
}

/// Check if a path is a symlink.
pub(crate) fn is_symlink(path: &Path) -> bool {
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
