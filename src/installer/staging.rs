//! Staged extraction and atomic binary replacement (DDL-4oi, SEC-01).

use super::*;
use crate::error::{DdlError, Result};
use std::path::{Path, PathBuf};

/// Extract the expected binary from `bytes` into exclusive staging, then
/// atomically replace the installed executable at `dest` (DDL-4oi, SEC-01).
///
/// Guarantees:
/// - `dest` is only ever mutated by a single atomic replacement of a fully
///   extracted, executable regular file — never by a partial write.
/// - On any failure the previous bytes at `dest` are preserved exactly and
///   no staging residue remains (`.ddl-stage-*` entries are removed).
/// - Only the expected binary entry is extracted: regular files only
///   (symlinks, hardlinks, directories and special files are rejected),
///   size-bounded by [`MAX_BINARY_BYTES`], with path traversal rejected.
///
/// Archive format is detected from the bytes (gzip magic → tar.gz, otherwise
/// zip) so both the unix and Windows asset paths share one code path.
pub fn stage_and_replace_binary(
    bytes: &[u8],
    dest: &Path,
    tool_name: &str,
    url: &str,
) -> Result<()> {
    let parent = dest
        .parent()
        .ok_or_else(|| DdlError::Other("destination has no parent directory".to_string()))?;
    // Exclusive staging directory, sibling of `dest` so the final swap is a
    // same-filesystem rename.
    let staging = new_staging_dir(parent, tool_name)?;

    let result = (|| -> Result<()> {
        let staged = if bytes.starts_with(&[0x1f, 0x8b]) {
            extract_tar_gz_staged(bytes, &staging, tool_name, url)?
        } else {
            extract_zip_staged(bytes, &staging, tool_name, url)?
        };
        make_executable(&staged)?;
        atomic_replace_executable(&staged, dest)
    })();

    if result.is_err() {
        // Normal failure cases leave zero staging residue behind.
        let _ = std::fs::remove_dir_all(&staging);
    } else {
        // The staged binary was moved into place; drop the staging shell.
        let _ = std::fs::remove_dir_all(&staging);
    }
    result
}

/// Create an exclusive staging directory (create-new semantics; the attempt
/// loop disambiguates same-nanosecond collisions) named `.ddl-stage-*` so
/// cleanup passes can recognize staging residue.
pub(crate) fn new_staging_dir(parent: &Path, tool_name: &str) -> Result<PathBuf> {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    for attempt in 0..64u32 {
        let dir = parent.join(format!(
            ".ddl-stage-{tool_name}-{}-{}",
            std::process::id(),
            nanos + attempt as u128
        ));
        match std::fs::create_dir(&dir) {
            Ok(()) => return Ok(dir),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(DdlError::Io(e)),
        }
    }
    Err(DdlError::InstallFailed(
        "could not create an exclusive staging directory for the upgrade".to_string(),
    ))
}

/// Extract only the expected regular-file binary entry from a tar.gz into
/// `staging`. Returns the staged file path. Links, special files, oversized
/// and traversal entries are rejected before any destination mutation.
/// Safety checks for one archive entry: regular-file-only, traversal guard,
/// and the size budget (DDL-4oi). Returns Err naming the violation.
fn validate_tar_entry(
    entry_path: &Path,
    entry_type: tar::EntryType,
    declared: u64,
    tool_name: &str,
    url: &str,
) -> Result<()> {
    let path_display = entry_path.display().to_string();
    // Only the expected regular binary is ever extracted.
    if !matches!(entry_type, tar::EntryType::Regular) {
        return Err(DdlError::InstallFailed(format!(
            "archive entry '{path_display}' for binary '{tool_name}' is not a regular file — rejected from {url}"
        )));
    }
    // Traversal guard: the staged copy is built manually, so the entry
    // path must stay inside the staging directory.
    if entry_path.components().any(|c| {
        matches!(
            c,
            std::path::Component::ParentDir
                | std::path::Component::RootDir
                | std::path::Component::Prefix(_)
        )
    }) {
        return Err(DdlError::InstallFailed(format!(
            "archive entry '{path_display}' attempts path traversal — rejected from {url}"
        )));
    }
    if declared > MAX_BINARY_BYTES {
        return Err(DdlError::InstallFailed(format!(
            "archive entry '{path_display}' declares {declared} bytes, exceeding the {MAX_BINARY_BYTES}-byte binary limit — rejected from {url}"
        )));
    }
    Ok(())
}

/// Copy one validated archive entry into staging, size-bounded; the staged
/// file is synced before the swap.
fn stage_tar_entry<R: std::io::Read>(
    entry: &mut tar::Entry<'_, R>,
    entry_path: &Path,
    declared: u64,
    staging: &Path,
    url: &str,
) -> Result<PathBuf> {
    let staged_path = staging.join(entry_path);
    if let Some(p) = staged_path.parent() {
        std::fs::create_dir_all(p).map_err(DdlError::Io)?;
    }
    let mut out = std::fs::File::create(&staged_path).map_err(DdlError::Io)?;
    let copied = std::io::copy(entry, &mut out).map_err(DdlError::Io)?;
    if copied != declared {
        let path_display = entry_path.display().to_string();
        return Err(DdlError::InstallFailed(format!(
            "interrupted copy: archive entry '{path_display}' declared {declared} bytes but only {copied} could be read — {url}"
        )));
    }
    out.sync_all().map_err(DdlError::Io)?;
    Ok(staged_path)
}

fn extract_tar_gz_staged(
    bytes: &[u8],
    staging: &Path,
    tool_name: &str,
    url: &str,
) -> Result<PathBuf> {
    let decoder = flate2::read::GzDecoder::new(bytes);
    let mut archive = tar::Archive::new(decoder);

    for entry in archive
        .entries()
        .map_err(|e| DdlError::Other(e.to_string()))?
    {
        let mut entry = entry.map_err(|e| DdlError::Other(e.to_string()))?;
        let path = entry
            .path()
            .map_err(|e| DdlError::Other(e.to_string()))?
            .to_path_buf();
        if path.file_name().is_none_or(|f| f != tool_name) {
            continue;
        }
        let declared = entry
            .header()
            .size()
            .map_err(|e| DdlError::Other(e.to_string()))?;
        validate_tar_entry(&path, entry.header().entry_type(), declared, tool_name, url)?;
        return stage_tar_entry(&mut entry, &path, declared, staging, url);
    }

    Err(DdlError::InstallFailed(format!(
        "Binary '{tool_name}' not found in archive from {url}"
    )))
}

/// Extract only the expected regular-file binary entry from a zip into
/// `staging`. Returns the staged file path. Symlinks, directories, oversized
/// and traversal entries are rejected before any destination mutation.
/// Reject non-regular zip entries (symlinks, directories) — only the
/// expected regular binary is ever extracted.
fn validate_zip_entry(
    entry: &zip::read::ZipFile<'_, std::io::Cursor<std::vec::Vec<u8>>>,
    name: &str,
    tool_name: &str,
    url: &str,
) -> Result<u64> {
    if entry.is_symlink() || entry.is_dir() {
        return Err(DdlError::InstallFailed(format!(
            "archive entry '{name}' for binary '{tool_name}' is not a regular file — rejected from {url}"
        )));
    }
    let declared = entry.size();
    if declared > MAX_BINARY_BYTES {
        return Err(DdlError::InstallFailed(format!(
            "archive entry '{name}' declares {declared} bytes, exceeding the {MAX_BINARY_BYTES}-byte binary limit — rejected from {url}"
        )));
    }
    Ok(declared)
}

/// Copy one validated zip entry into staging (traversal-safe via
/// enclosed_name), size-bounded, synced before the swap.
fn stage_zip_entry(
    entry: &mut zip::read::ZipFile<'_, std::io::Cursor<std::vec::Vec<u8>>>,
    name: &str,
    declared: u64,
    staging: &Path,
    url: &str,
) -> Result<PathBuf> {
    // enclosed_name() rejects traversal ("../", absolute paths).
    let staged_path = staging.join(entry.enclosed_name().ok_or_else(|| {
        DdlError::InstallFailed(format!(
            "archive entry '{name}' attempts path traversal — rejected from {url}"
        ))
    })?);
    if let Some(p) = staged_path.parent() {
        std::fs::create_dir_all(p).map_err(DdlError::Io)?;
    }
    let mut out = std::fs::File::create(&staged_path).map_err(DdlError::Io)?;
    let copied = std::io::copy(entry, &mut out).map_err(DdlError::Io)?;
    if copied != declared {
        return Err(DdlError::InstallFailed(format!(
            "interrupted copy: archive entry '{name}' declared {declared} bytes but only {copied} could be read — {url}"
        )));
    }
    out.sync_all().map_err(DdlError::Io)?;
    Ok(staged_path)
}

pub(crate) fn extract_zip_staged(
    bytes: &[u8],
    staging: &Path,
    tool_name: &str,
    url: &str,
) -> Result<PathBuf> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes.to_vec()))
        .map_err(|e| DdlError::Other(e.to_string()))?;

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| DdlError::Other(e.to_string()))?;
        let name = entry
            .name()
            .map_err(|e| DdlError::Other(e.to_string()))?
            .into_owned();
        let base = name
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .unwrap_or(&name);
        if base != tool_name {
            continue;
        }
        let declared = validate_zip_entry(&entry, &name, tool_name, url)?;
        return stage_zip_entry(&mut entry, &name, declared, staging, url);
    }

    Err(DdlError::InstallFailed(format!(
        "Binary '{tool_name}' not found in zip archive from {url}"
    )))
}

/// Mark a staged binary executable (POSIX mode 0o755; a no-op on Windows,
/// where executability is carried by the file extension).
pub(crate) fn make_executable(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
            .map_err(DdlError::Io)?;
    }
    Ok(())
}

/// Atomically replace the installed executable at `dest` with the staged
/// binary (DDL-4oi).
///
/// - POSIX: `rename(2)` is atomic over an existing file — a concurrent
///   reader observes either the complete old or the complete new executable,
///   never a partial write.
/// - Windows: `std::fs::rename` fails when `dest` exists, so the replacement
///   is a defined move-aside protocol with rollback: (1) move the current
///   executable to `<dest>.ddl-old`, (2) rename the staged binary into
///   place, (3) delete the rollback file. Any failure in (2) restores the
///   old executable; concurrent upgrades are serialized by the DDL-5bs
///   install lock, closing the brief exposure window between (1) and (2).
pub(crate) fn atomic_replace_executable(staged: &Path, dest: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        std::fs::rename(staged, dest).map_err(DdlError::Io)?;
        Ok(())
    }
    #[cfg(not(unix))]
    {
        match std::fs::rename(staged, dest) {
            Ok(()) => Ok(()),
            Err(_) => {
                let bak = sibling_path(dest, ".ddl-old");
                let _ = std::fs::remove_file(&bak);
                std::fs::rename(dest, &bak).map_err(DdlError::Io)?;
                match std::fs::rename(staged, dest) {
                    Ok(()) => {
                        let _ = std::fs::remove_file(&bak);
                        Ok(())
                    }
                    Err(e) => {
                        // Rollback: the working executable must survive.
                        let _ = std::fs::rename(&bak, dest);
                        Err(DdlError::Io(e))
                    }
                }
            }
        }
    }
}

/// Path for a sibling of `dest` with `suffix` appended to the file name.
#[cfg(not(unix))]
pub(crate) fn sibling_path(dest: &Path, suffix: &str) -> PathBuf {
    let mut name = dest
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_default();
    name.push(suffix);
    match dest.parent() {
        Some(p) => p.join(name),
        None => PathBuf::from(name),
    }
}
