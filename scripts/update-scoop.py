#!/usr/bin/env python3
"""Create or update a Scoop manifest for dulce-de-leche.

Usage:
    python3 scripts/update-scoop.py <manifest_path> <version> <tag> <checksums_path>

Example:
    python3 scripts/update-scoop.py /tmp/bucket/dulce-de-leche.json 0.1.0 v0.1.0 dist/checksums.txt

Purpose:
    Renders the scoop-charly bucket manifest for one release. DDL-9e7: the
    script refuses to write any output unless the declared version, the
    asset tag, and the windows_amd64 checksum entry correspond — a failed
    verification exits non-zero and leaves the target untouched, so a
    retry can never commit a half-verified channel update. Rendering is
    deterministic, so re-running against an already-updated bucket is a
    no-op (idempotent retry).
"""
import json
import os
import sys


class VerificationError(Exception):
    """Release inputs do not correspond; the channel update is refused."""


def verify_release_inputs(version, tag, checksums_path):
    """Assert declared version, asset tag, and checksum correspond.

    Raises VerificationError (before any file is written) when:
      * the tag does not equal "v" + version, or
      * the windows_amd64 asset is missing from the checksum file.
    """
    if tag != "v" + version:
        raise VerificationError(
            f"refusing channel update: tag {tag!r} does not match "
            f"declared version {version!r} (expected 'v{version}')"
        )
    if not os.path.isfile(checksums_path):
        raise VerificationError(
            f"refusing channel update: checksums file {checksums_path!r} not found"
        )
    sha_win = parse_checksums(checksums_path)
    if sha_win is None:
        raise VerificationError(
            "refusing channel update: windows_amd64 checksum not found in "
            f"{checksums_path!r}"
        )
    return sha_win


def parse_checksums(checksums_path):
    """Return the windows_amd64 sha256, or None when absent."""
    sha_win = None
    with open(checksums_path) as f:
        for line in f:
            parts = line.split()
            if len(parts) == 2 and "windows_amd64" in parts[1]:
                sha_win = parts[0]
                break
    return sha_win


def build_manifest(version, tag, sha_win):
    """Build the manifest dict deterministically."""
    url = (
        "https://github.com/charly-vibes/dulce-de-leche/releases/download/"
        f"{tag}/ddl_{version}_windows_amd64.zip"
    )
    return {
        "version": version,
        "description": "Orchestrator for the charly-vibes tool ecosystem",
        "homepage": "https://github.com/charly-vibes/dulce-de-leche",
        "license": "Apache-2.0",
        "url": url,
        "hash": f"sha256:{sha_win}",
        "bin": "ddl.exe",
        "checkver": {
            "github": "https://github.com/charly-vibes/dulce-de-leche"
        },
        "autoupdate": {
            "url": "https://github.com/charly-vibes/dulce-de-leche/releases/download/v$version/ddl_$version_windows_amd64.zip"
        }
    }


def write_manifest(manifest_path, manifest):
    os.makedirs(os.path.dirname(manifest_path), exist_ok=True)
    with open(manifest_path, "w") as f:
        json.dump(manifest, f, indent=4)
        f.write("\n")


def main(argv):
    manifest_path, version, tag, checksums_path = argv[1], argv[2], argv[3], argv[4]
    try:
        sha_win = verify_release_inputs(version, tag, checksums_path)
    except VerificationError as exc:
        print(f"ERROR: {exc}", file=sys.stderr)
        return 2
    manifest = build_manifest(version, tag, sha_win)
    write_manifest(manifest_path, manifest)
    print(f"Wrote {manifest_path} (version {version})")
    print(f"  url: {manifest['url']}")
    print(f"  sha256: {sha_win[:16]}...")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
