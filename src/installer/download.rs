//! Checksum verification and release-asset download with size budgets (DDL-5ph, DDL-1z1).

use super::*;
use crate::error::{DdlError, Result};
use crate::platform::Tool;

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        hex.push_str(&format!("{byte:02x}"));
    }
    hex
}

/// Parse a shasum-style checksum manifest (`<sha256>  <filename>` per line;
/// two spaces per `shasum -a 256`, single space and bsd-style `"<hex>" *name`
/// also accepted). Lines without a 64-char hex digest are skipped.
/// Returns `None` when nothing valid remains.
pub(crate) fn parse_checksums(content: &str) -> Option<std::collections::HashMap<String, String>> {
    let mut map = std::collections::HashMap::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        // bsd-style: "<hex>" *filename
        let (digest, name) = if let Some(quoted) = line.strip_prefix('"') {
            let Some((digest, rest)) = quoted.split_once('"') else {
                continue;
            };
            let rest = rest.trim_start();
            let name = rest.trim_start_matches('*');
            (digest, name)
        } else {
            let Some((digest, rest)) = line.split_once(char::is_whitespace) else {
                continue;
            };
            // sha256sum binary mode prefixes the filename with `*`
            (digest, rest.trim_start().trim_start_matches('*'))
        };
        let digest = digest.to_lowercase();
        if digest.len() != 64 || !digest.chars().all(|c| c.is_ascii_hexdigit()) {
            continue;
        }
        if name.is_empty() {
            continue;
        }
        map.insert(name.to_string(), digest);
    }
    (!map.is_empty()).then_some(map)
}

/// Verify `bytes` hash to `expected` (case-insensitive). Mismatch is a hard
/// `ChecksumMismatch` naming the tool — the caller must not install the asset.
pub(crate) fn verify_checksum(bytes: &[u8], expected: &str, tool: &str, url: &str) -> Result<()> {
    let actual = sha256_hex(bytes);
    if actual == expected.to_lowercase() {
        Ok(())
    } else {
        Err(DdlError::ChecksumMismatch {
            tool: tool.to_string(),
            expected: expected.to_lowercase(),
            actual,
            url: url.to_string(),
        })
    }
}

/// Look up the published sha256 for the asset at `archive_url`, reporting the
/// exact reason whenever verification is not possible (DDL-yts).
///
/// Conventions checked in order:
/// 1. `checksums.txt` next to the asset (shasum format — ddl's own release
///    workflow publishes this), with the archive looked up by filename
/// 2. a per-asset `<archive>.sha256` file containing a bare digest
///
/// Returns [`ChecksumLookup::Unavailable`] — with the precise reason — when
/// the release publishes no usable checksum source, the metadata is
/// malformed, the asset is absent from the manifest, or the metadata fetch
/// itself fails. Missing/malformed/transport-failed are all rejections under
/// the DDL-yts policy (see `enforce_checksum_policy`), not skip-and-install.
pub(crate) fn fetch_expected_checksum(archive_url: &str, _verbose: bool) -> ChecksumLookup {
    let Some((base, archive_name)) = archive_url.rsplit_once('/') else {
        return ChecksumLookup::Unavailable("asset URL has no parent path");
    };

    let checksums_url = format!("{base}/checksums.txt");
    match reqwest::blocking::get(&checksums_url) {
        Ok(resp) if resp.status().is_success() => {
            if let Ok(body) = resp.text() {
                match parse_checksums(&body) {
                    Some(map) => {
                        if let Some(expected) = map.get(archive_name).cloned() {
                            return ChecksumLookup::Published(expected);
                        }
                        return ChecksumLookup::Unavailable(
                            "asset is not listed in this release's checksums.txt",
                        );
                    }
                    None => {
                        return ChecksumLookup::Unavailable("checksums.txt is malformed");
                    }
                }
            }
            return ChecksumLookup::Unavailable("checksums.txt body could not be read");
        }
        Ok(_) => {
            // checksums.txt answered but not with success — fall through to
            // the per-asset convention.
        }
        Err(_) => {
            // Metadata transport failed — an attacker or outage must not be
            // indistinguishable from "no checksum published". Rejected.
            return ChecksumLookup::Unavailable("checksums.txt could not be fetched");
        }
    }

    let per_asset_url = format!("{archive_url}.sha256");
    if let Ok(resp) = reqwest::blocking::get(&per_asset_url)
        && resp.status().is_success()
        && let Ok(body) = resp.text()
    {
        let digest = body.split_whitespace().next().unwrap_or("").to_lowercase();
        if digest.len() == 64 && digest.chars().all(|c| c.is_ascii_hexdigit()) {
            return ChecksumLookup::Published(digest);
        }
        return ChecksumLookup::Unavailable("per-asset .sha256 file is malformed");
    }

    ChecksumLookup::Unavailable("release publishes no checksum for the asset")
}

/// Verify a downloaded release archive under the DDL-yts policy.
///
/// Missing / malformed / incomplete / transport-failed checksum metadata is a
/// rejection (see `enforce_checksum_policy`); the only bypass is the explicit
/// `DDL_ALLOW_UNVERIFIED_RELEASE=1` authorization, which yields
/// [`VerificationStatus::UnverifiedAuthorized`]. Rejection happens before any
/// destination mutation — the caller must not extract when this returns Err.
pub(crate) fn verify_download(
    archive_url: &str,
    archive_bytes: &[u8],
    tool: &Tool,
    verbose: bool,
) -> Result<VerificationStatus> {
    let checksum = fetch_expected_checksum(archive_url, verbose);
    // DDL-wof identity applies to charly-vibes origins only; external origins
    // verify through the release's own checksums (DDL-9x4.11.1), so the
    // provenance fetch is skipped there — a request that could never succeed
    // (they publish no provenance.json) is not issued.
    let provenance = if charly_vibes_origin(archive_url) {
        fetch_provenance(archive_url, verbose)
    } else {
        ProvenanceLookup::Unavailable("not fetched (external origin)")
    };
    verify_release_bytes(
        tool.name,
        archive_url,
        checksum,
        provenance,
        archive_bytes,
        unverified_authorized(),
    )
}

/// Fetch a release archive, mapping the release-404
/// case to `NoReleaseBinary` and any other non-success to `InstallFailed`.
pub(crate) fn fetch_release_asset(url: &str, tool_name: &str) -> Result<bytes::Bytes> {
    use std::io::Read;
    let response = reqwest::blocking::get(url).map_err(DdlError::Network)?;

    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(DdlError::NoReleaseBinary {
            tool: tool_name.to_string(),
            url: url.to_string(),
        });
    }
    if !response.status().is_success() {
        return Err(DdlError::InstallFailed(format!(
            "Download failed: HTTP {} for {}",
            response.status(),
            url
        )));
    }
    // DDL-1z1: the release body is bounded by MAX_RELEASE_ASSET_BYTES. The
    // declared Content-Length is rejected before the first byte is read; the
    // streamed byte count is re-checked as chunks arrive (covers the
    // unknown-length case). The body is never buffered past the budget.
    enforce_download_budget(response.content_length(), 0)?;
    let mut response = response;
    let mut body: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 64 * 1024];
    // `blocking::Response` implements `std::io::Read` (decoded bytes), so the
    // streamed count is re-checked as data arrives — the unknown-length case
    // is still bounded.
    loop {
        let n = response
            .read(&mut chunk)
            .map_err(|e| DdlError::InstallFailed(format!("Download failed: {e} for {url}")))?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..n]);
        enforce_download_budget(None, body.len() as u64)?;
    }
    Ok(body.into())
}
