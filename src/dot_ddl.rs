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

/// True for Windows sharing/lock violations (os errors 32/33) — transient
/// locks held by antivirus scanners or other processes on freshly-written
/// files. Always false elsewhere: those raw codes don't occur on Unix
/// (Linux 32 is EPIPE, which a plain file write cannot produce).
fn is_sharing_violation(e: &DdlError) -> bool {
    matches!(e, DdlError::Io(io) if matches!(io.raw_os_error(), Some(32) | Some(33)))
}

/// SEC-01 (DDL-8u8): atomically replace the manifest file.
///
/// Baseline (pre-DDL-8u8) wrote through the locked manifest handle after
/// truncating it (`set_len(0)`): a crash in that window left an empty file
/// — which [`Manifest::parse`] then accepted as a fresh manifest, silently
/// resetting established tool state — and a crash mid-write left partial
/// JSON on disk.
///
/// Replacement strategy: stage the serialized manifest in a temp sibling
/// file, fsync it, then `rename()` over the target. POSIX rename is atomic,
/// so readers observe either the complete old or the complete new manifest
/// — never partial JSON — and a failure anywhere before the rename leaves
/// the prior manifest byte-for-byte intact. On Windows `std::fs::rename`
/// uses MOVEFILE_REPLACE_EXISTING; transient sharing violations during the
/// rename are retried by the caller's [`with_sharing_retry`].
///
/// The staging file is opened no-follow (SEC-05): a link swapped in at the
/// temp path cannot redirect the staged write.
fn stage_manifest(manifest_path: &Path, manifest: &Manifest) -> Result<PathBuf> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

    let dir = manifest_path.parent().ok_or_else(|| {
        DdlError::Other(format!(
            "manifest path {} has no parent directory",
            manifest_path.display()
        ))
    })?;
    let stem = manifest_path
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_else(|| "manifest.json".into());
    let counter = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let tmp_path = dir.join(format!(
        "{}.tmp.{}.{}",
        stem.to_string_lossy(),
        std::process::id(),
        counter
    ));

    let contents = serde_json::to_string_pretty(manifest).map_err(DdlError::Serde)?;
    let mut file = open_manifest_no_follow(&tmp_path)?;
    file.write_all(contents.as_bytes()).map_err(DdlError::Io)?;
    // fsync the staged file so the rename cannot publish zero-length or
    // torn contents after a crash.
    file.sync_all().map_err(DdlError::Io)?;
    Ok(tmp_path)
}

/// Replace `manifest_path` with the already-staged file, then (Unix)
/// best-effort fsync the parent directory so the rename itself survives a
/// power loss. Best-effort: a failed directory fsync degrades durability
/// but never loses the on-disk new state.
fn commit_staged(tmp_path: &Path, manifest_path: &Path) -> Result<()> {
    if let Err(e) = std::fs::rename(tmp_path, manifest_path) {
        // Leave no staging debris behind on a failed rename.
        let _ = std::fs::remove_file(tmp_path);
        return Err(DdlError::Io(e));
    }
    sync_parent_dir(manifest_path);
    Ok(())
}

/// Write the manifest as an atomic replace (SEC-01, DDL-8u8). See
/// [`stage_manifest`] for the durability rationale.
fn write_manifest_atomic(manifest_path: &Path, manifest: &Manifest) -> Result<()> {
    let tmp_path = stage_manifest(manifest_path, manifest)?;
    commit_staged(&tmp_path, manifest_path)
}

/// Path of the dedicated manifest lock file (a stable sibling that is
/// never renamed or replaced).
fn manifest_lock_path(manifest_path: &Path) -> PathBuf {
    let mut name = manifest_path
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_else(|| "manifest.json".into());
    name.push(".lock");
    manifest_path.with_file_name(name)
}

/// Acquire the manifest lock (SEC-01, DDL-8u8).
///
/// Serialization uses a dedicated lock file rather than a lock on the
/// manifest itself: the lock file is never renamed or replaced, so the
/// lock can be held through the atomic rename without the Windows
/// rename-over-locked-open-file interaction (the DDL-71i class of
/// self-conflicts). The lock file is opened no-follow (SEC-05).
/// Dropped when the returned handle goes out of scope.
fn manifest_lock_guard(manifest_path: &Path) -> Result<std::fs::File> {
    let lock_path = manifest_lock_path(manifest_path);
    let file = open_manifest_no_follow(&lock_path)?;
    file.lock_exclusive().map_err(DdlError::Io)?;
    Ok(file)
}

/// Best-effort fsync of the directory containing `path` (Unix only).
#[cfg(unix)]
fn sync_parent_dir(path: &Path) {
    if let Some(dir) = path.parent() {
        let _ = std::fs::File::open(dir).map(|f| f.sync_all());
    }
}

/// Best-effort directory fsync is a Unix durability nicety; Windows
/// relies on MOVEFILE_REPLACE_EXISTING journaling semantics.
#[cfg(not(unix))]
fn sync_parent_dir(_path: &Path) {}

/// Run a manifest write operation, retrying transient Windows file locks
/// (DDL-71i). Windows LockFileEx locks held by antivirus scanners or other
/// processes block our open/lock step; exponential backoff (6 attempts,
/// ~6.3s worst case) rides those out without slowing down real failures.
fn with_sharing_retry<F>(mut op: F) -> Result<()>
where
    F: FnMut() -> Result<()>,
{
    const MAX_ATTEMPTS: u32 = 6;
    for attempt in 1..=MAX_ATTEMPTS {
        match op() {
            Ok(()) => return Ok(()),
            Err(e) if is_sharing_violation(&e) && attempt < MAX_ATTEMPTS => {
                std::thread::sleep(std::time::Duration::from_millis(
                    100 * 2u64.pow(attempt - 1),
                ));
            }
            Err(e) => return Err(e),
        }
    }
    unreachable!("retry loop always returns")
}

/// SEC-05 (DDL-sg8): refuse manifest writes that links could redirect
/// outside the project.
///
/// Two checks run before every manifest write path:
/// 1. The manifest path itself must not be a symlink (checked via
///    `symlink_metadata`, so dangling links are caught too). Even an
///    in-project link is untrusted: it can be swapped between check and
///    write, and a write through it lands at the target, wherever that is.
/// 2. The `.ddl/` directory must not be a link pointing outside the project
///    root. In-project directory links (e.g. `.ddl` -> `.internal-ddl`)
///    stay allowed — they cannot escape the tree.
///
/// Rationale (DDL-sg8): these checks are companions to the real enforcement
/// in [`open_manifest_no_follow`], which refuses to follow a link at the
/// manifest path inside the `open()` itself — closing the check-then-open
/// race these path-based checks inherently have.
///
/// Missing/dangling ancestors degrade to "allow": the subsequent `open()`
/// then fails (or creates) naturally, mirroring today's behavior for
/// genuinely absent directories; the guard only rejects link-mediated
/// redirection.
pub fn ensure_manifest_write_is_local(manifest_path: &Path) -> Result<()> {
    if is_symlink(manifest_path) {
        return Err(DdlError::Other(format!(
            "refusing to write manifest: {} is a symlink (SEC-05: manifest links must not redirect writes outside the project)",
            manifest_path.display()
        )));
    }

    let Some(ddl_dir) = manifest_path.parent() else {
        return Ok(());
    };
    let Some(root) = ddl_dir.parent() else {
        return Ok(());
    };
    let (Ok(root), Ok(resolved)) = (std::fs::canonicalize(root), std::fs::canonicalize(ddl_dir))
    else {
        return Ok(());
    };
    if !resolved.starts_with(&root) {
        return Err(DdlError::Other(format!(
            "refusing to write manifest: {} links to {}, outside the project root {} (SEC-05)",
            ddl_dir.display(),
            resolved.display(),
            root.display()
        )));
    }
    Ok(())
}

/// Open the manifest for read/write without following a link at the final
/// path component (SEC-05, DDL-sg8).
///
/// [`ensure_manifest_write_is_local`] runs before every open, but a check
/// and a later open are two syscalls: a link swapped in between them would
/// still be followed. Opening with `O_NOFOLLOW` (Unix) resp.
/// `FILE_FLAG_OPEN_REPARSE_POINT` (Windows) makes the refusal happen inside
/// the open itself, so the last line of defense cannot be raced past.
/// Dangling links are refused too: `O_NOFOLLOW` + `O_CREAT` fails with ELOOP
/// on a trailing symlink.
fn open_manifest_no_follow(path: &Path) -> Result<std::fs::File> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let mut opts = OpenOptions::new();
        opts.create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .custom_flags(libc::O_NOFOLLOW);
        opts.open(path).map_err(|e| symlink_open_error(path, e))
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // FILE_FLAG_OPEN_REPARSE_POINT: open the reparse point itself
        // instead of following it to the target.
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        let mut opts = OpenOptions::new();
        opts.create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
        opts.open(path).map_err(|e| symlink_open_error(path, e))
    }
    #[cfg(not(any(unix, windows)))]
    {
        OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(path)
            .map_err(|e| symlink_open_error(path, e))
    }
}

/// Map a failed no-follow open to a user-facing error. When the path is (or
/// now is) a link, name it explicitly; otherwise surface the raw error.
fn symlink_open_error(path: &Path, e: std::io::Error) -> DdlError {
    if is_symlink(path) {
        DdlError::Other(format!(
            "refusing to open manifest: {} is a symlink (SEC-05: manifest links must not redirect writes outside the project)",
            path.display()
        ))
    } else {
        DdlError::Io(e)
    }
}

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
    ("vampiro", ".vampiro"),
    ("specodelic", ".specs"),
    ("funkalization", ".funkalization"),
    // Note: bd, openspec, incitaciones, and turu are intentionally absent —
    // they are install-only (bd manages repo-local `.beads/` itself, openspec
    // manages `openspec/` itself, skills land in `~/.agents/skills/`, and
    // turu's state is home-dir global). See docs/ecosystem-map.md.
];

/// The legacy tool config directories, as `(binary name, legacy path)` pairs.
/// Tools without a legacy config (e.g. `bd`, `openspec`) are absent.
pub fn legacy_configs() -> &'static [(&'static str, &'static str)] {
    LEGACY_CONFIGS
}

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

#[cfg(test)]
mod sec05_tests {
    use super::*;

    /// SEC-05 (DDL-sg8), link-swap coverage: after the swap from a regular
    /// manifest to a link, the no-follow open itself refuses — no prior
    /// check is involved, so the check-then-open race window cannot be used
    /// to get a write through.
    #[test]
    fn no_follow_open_refuses_manifest_swapped_to_symlink() {
        let dir = tempfile();
        let manifest = dir.path().join("manifest.json");
        std::fs::write(&manifest, "{}").unwrap();

        // Regular file: open succeeds (plain save path).
        open_manifest_no_follow(&manifest).unwrap();

        // Swap in a link pointing at an outside file; the open itself must
        // refuse even though no path-based check has run since the swap.
        let outside = dir.path().join("outside.json");
        std::fs::write(&outside, "harmless").unwrap();
        #[cfg(unix)]
        {
            std::fs::remove_file(&manifest).unwrap();
            std::os::unix::fs::symlink(&outside, &manifest).unwrap();
        }
        #[cfg(windows)]
        {
            std::fs::remove_file(&manifest).unwrap();
            std::os::windows::fs::symlink_file(&outside, &manifest)
                .expect("symlink_file requires admin/dev-mode privileges");
        }

        let err = open_manifest_no_follow(&manifest).unwrap_err();
        assert!(err.to_string().contains("symlink"), "got: {err}");
        // The outside file must be untouched.
        assert_eq!(std::fs::read_to_string(&outside).unwrap(), "harmless");
    }

    /// SEC-05 (DDL-sg8): a dangling link at the manifest path is refused by
    /// the no-follow open, and the dangling target is not created.
    #[cfg(unix)]
    #[test]
    fn no_follow_open_refuses_dangling_link() {
        let dir = tempfile();
        let manifest = dir.path().join("manifest.json");
        std::os::unix::fs::symlink(dir.path().join("nowhere.json"), &manifest).unwrap();

        let err = open_manifest_no_follow(&manifest).unwrap_err();
        assert!(err.to_string().contains("symlink"), "got: {err}");
        assert!(!dir.path().join("nowhere.json").exists());
    }

    /// SEC-05 (DDL-sg8): the no-follow open must not have regressed to
    /// following links — a link to an in-project file is still refused,
    /// because any manifest link can be swapped between check and write.
    #[cfg(unix)]
    #[test]
    fn no_follow_open_refuses_even_in_project_link() {
        let dir = tempfile();
        let manifest = dir.path().join("manifest.json");
        let inside = dir.path().join("inside.json");
        std::fs::write(&inside, "keep").unwrap();
        std::os::unix::fs::symlink(&inside, &manifest).unwrap();

        assert!(open_manifest_no_follow(&manifest).is_err());
        assert_eq!(std::fs::read_to_string(&inside).unwrap(), "keep");
    }

    fn tempfile() -> tempfile::TempDir {
        tempfile::Builder::new()
            .prefix("ddl-sec05")
            .tempdir()
            .expect("tempdir")
    }
}
