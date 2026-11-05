//! Manifest persistence primitives: atomic staged writes, locking, sharing-violation retry, symlink-safe open (SEC hardening).

use super::*;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::error::{DdlError, Result};
use crate::manifest::Manifest;
use fs2::FileExt;

pub(crate) fn is_sharing_violation(e: &DdlError) -> bool {
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
pub(crate) fn stage_manifest(manifest_path: &Path, manifest: &Manifest) -> Result<PathBuf> {
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
pub(crate) fn commit_staged(tmp_path: &Path, manifest_path: &Path) -> Result<()> {
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
pub(crate) fn write_manifest_atomic(manifest_path: &Path, manifest: &Manifest) -> Result<()> {
    let tmp_path = stage_manifest(manifest_path, manifest)?;
    commit_staged(&tmp_path, manifest_path)
}

/// Path of the dedicated manifest lock file (a stable sibling that is
/// never renamed or replaced).
pub(crate) fn manifest_lock_path(manifest_path: &Path) -> PathBuf {
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
pub(crate) fn manifest_lock_guard(manifest_path: &Path) -> Result<std::fs::File> {
    let lock_path = manifest_lock_path(manifest_path);
    let file = open_manifest_no_follow(&lock_path)?;
    file.lock_exclusive().map_err(DdlError::Io)?;
    Ok(file)
}

/// Best-effort fsync of the directory containing `path` (Unix only).
#[cfg(unix)]
pub(crate) fn sync_parent_dir(path: &Path) {
    if let Some(dir) = path.parent() {
        let _ = std::fs::File::open(dir).map(|f| f.sync_all());
    }
}

/// Best-effort directory fsync is a Unix durability nicety; Windows
/// relies on MOVEFILE_REPLACE_EXISTING journaling semantics.
#[cfg(not(unix))]
pub(crate) fn sync_parent_dir(_path: &Path) {}

/// Run a manifest write operation, retrying transient Windows file locks
/// (DDL-71i). Windows LockFileEx locks held by antivirus scanners or other
/// processes block our open/lock step; exponential backoff (6 attempts,
/// ~6.3s worst case) rides those out without slowing down real failures.
pub(crate) fn with_sharing_retry<F>(mut op: F) -> Result<()>
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
pub(crate) fn open_manifest_no_follow(path: &Path) -> Result<std::fs::File> {
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
pub(crate) fn symlink_open_error(path: &Path, e: std::io::Error) -> DdlError {
    if is_symlink(path) {
        DdlError::Other(format!(
            "refusing to open manifest: {} is a symlink (SEC-05: manifest links must not redirect writes outside the project)",
            path.display()
        ))
    } else {
        DdlError::Io(e)
    }
}
