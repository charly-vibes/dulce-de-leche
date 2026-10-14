#!/usr/bin/env bash
# Purpose: stable, single-URL bootstrap entry point for dulce-de-leche (ddl)
#          on macOS & Linux — replaces the 4-line curl|tar snippet in
#          README.md / llms.txt (DDL-1ay).
# Responsibilities:
#   - Resolve the latest (or pinned) release, download the pinned archive
#     AND the release's checksums.txt.
#   - Verify the archive sha256 against checksums.txt BEFORE any
#     extraction or execution, mirroring the DDL-yts policy in
#     src/installer.rs (`enforce_checksum_policy`): a missing, malformed,
#     unfetchable checksums.txt or an asset not listed in it is a
#     fail-closed rejection — zero executable installs.
#   - The ONLY bypass is explicit authorization via
#     `DDL_ALLOW_UNVERIFIED_RELEASE=1` (same env var and semantics as the
#     Rust installer); a checksum MISMATCH is never bypassed.
#   - Extract into a temp dir and install the `ddl` binary into
#     DDL_INSTALL_DIR (default /usr/local/bin).
# Rationale: SEC-03 — no curl-to-execution path without validation. The
#   stable URL alone is insufficient; this script pins the
#   verify-before-extract contract at shell level.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/charly-vibes/dulce-de-leche/main/install.sh | bash
#
# Environment overrides (also used by tests/install_sh_fixtures.sh):
#   DDL_VERSION                   pin a release version (e.g. 0.8.0), skip "latest" resolution
#   DDL_BASE_URL                  base URL for release assets (default: GitHub releases/download/v$V)
#   DDL_INSTALL_DIR               destination dir (default: /usr/local/bin)
#   DDL_ALLOW_UNVERIFIED_RELEASE  set to 1 to explicitly authorize installing
#                                 when no usable checksum is published (legacy transition)
set -euo pipefail

REPO="charly-vibes/dulce-de-leche"
WORK_DIR="$(mktemp -d)"
trap 'rm -rf "$WORK_DIR"' EXIT

# DDL-yts policy escape hatch, shared by every fail-closed rejection path.
# Mirrors `enforce_checksum_policy` in src/installer.rs: an unusable checksum
# source is only installable with EXPLICIT authorization; a checksum MISMATCH
# is never bypassed (handled separately below).
authorized_unverified() {
    [[ "${DDL_ALLOW_UNVERIFIED_RELEASE:-}" == "1" ]]
}

# Only called after verification has accepted (or explicitly authorized) the
# archive. Extracts into the temp dir — never directly into the destination —
# and installs the verified binary.
install_verified() {
    local dest_dir="${DDL_INSTALL_DIR:-/usr/local/bin}"
    tar -xzf "$ARCHIVE_PATH" -C "$WORK_DIR"
    if [[ ! -x "$WORK_DIR/ddl" ]]; then
        echo "error: verified archive did not contain an executable \`ddl\` binary" >&2
        exit 1
    fi
    if [[ -w "$dest_dir" ]]; then
        mv -f "$WORK_DIR/ddl" "$dest_dir/ddl"
    else
        sudo mv -f "$WORK_DIR/ddl" "$dest_dir/ddl"
    fi
    chmod +x "$dest_dir/ddl"
    echo "installed ddl ${VERSION} to ${dest_dir}/ddl"
    "$dest_dir/ddl" --version || true
}

# Resolve the release version: pinned via DDL_VERSION, else the /releases/latest
# redirect target (`.../tag/v<version>`).
if [[ -n "${DDL_VERSION:-}" ]]; then
    VERSION="$DDL_VERSION"
else
    latest_url="$(curl -fsSLI -o /dev/null -w '%{url_effective}' \
        "https://github.com/${REPO}/releases/latest")"
    VERSION="$(basename "$latest_url")"
    VERSION="${VERSION#v}"
fi

BASE_URL="${DDL_BASE_URL:-https://github.com/${REPO}/releases/download/v${VERSION}}"
OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
ARCH="$(uname -m | sed 's/^x86_64$/amd64/; s/^aarch64$/arm64/')"
ARCHIVE_NAME="ddl_${VERSION}_${OS}_${ARCH}.tar.gz"
ARCHIVE_URL="${BASE_URL}/${ARCHIVE_NAME}"
CHECKSUMS_URL="${BASE_URL}/checksums.txt"

# 1. Download the archive bytes into the temp dir (no extraction yet).
ARCHIVE_PATH="$WORK_DIR/$ARCHIVE_NAME"
curl -fsSL "$ARCHIVE_URL" -o "$ARCHIVE_PATH"

# 2. DDL-yts verification policy (shell mirror of `enforce_checksum_policy`).
#    Fail-closed: any unusable checksum source aborts BEFORE extraction or
#    execution, so a tampered or unverified archive can never produce an
#    installed executable.
checksum_body="$(curl -fsSL "$CHECKSUMS_URL" 2>/dev/null)" || {
    if authorized_unverified; then
        echo "⚠ installing WITHOUT checksum verification (checksums.txt could not be fetched): explicitly authorized via DDL_ALLOW_UNVERIFIED_RELEASE" >&2
        install_verified
        exit 0
    fi
    echo "error: no usable checksum published for this release (checksums.txt could not be fetched); refusing to install (DDL-yts policy). Set DDL_ALLOW_UNVERIFIED_RELEASE=1 to explicitly authorize an unverified install." >&2
    exit 1
}

# Parse shasum-style manifest (`<sha256>  <filename>` per line) and look up
# the digest for THIS asset — same parsing as `parse_checksums` in
# src/installer.rs (sha256sum binary mode `*` prefix tolerated).
expected="$(awk -v asset="$ARCHIVE_NAME" '
    $1 ~ /^[0-9a-fA-F]{64}[*]?$/ && $2 != "" {
        name = $2; sub(/^\*/, "", name);
        if (name == asset) { print $1; found = 1; exit }
    }
    END { if (!found) exit 3 }' <<<"$checksum_body")" || {
    if authorized_unverified; then
        echo "⚠ installing WITHOUT checksum verification (asset not listed or checksums.txt malformed): explicitly authorized via DDL_ALLOW_UNVERIFIED_RELEASE" >&2
        install_verified
        exit 0
    fi
    echo "error: ${ARCHIVE_NAME} is not listed in this release's checksums.txt (or the manifest is malformed); refusing to install (DDL-yts policy). Set DDL_ALLOW_UNVERIFIED_RELEASE=1 to explicitly authorize an unverified install." >&2
    exit 1
}

# A published digest must MATCH — a mismatch is never bypassed by
# DDL_ALLOW_UNVERIFIED_RELEASE (parity with ChecksumMismatch in installer.rs).
actual="$(sha256sum "$ARCHIVE_PATH" 2>/dev/null | awk '{print $1}' \
    || shasum -a 256 "$ARCHIVE_PATH" | awk '{print $1}')"
if [[ "$actual" != "$expected" ]]; then
    echo "error: checksum mismatch for ${ARCHIVE_NAME}: expected ${expected}, got ${actual}; refusing to install (possible tampering). This is never bypassed by DDL_ALLOW_UNVERIFIED_RELEASE." >&2
    exit 1
fi
echo "✓ ${ARCHIVE_NAME} sha256 verified against release checksum"

# 3. Verified — only now extract and install.
install_verified
