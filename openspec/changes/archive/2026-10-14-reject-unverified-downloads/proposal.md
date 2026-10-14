# Change: Reject release downloads when required verification is unavailable

## Why

Security evaluation 2026-10-09, finding SEC-03 (DDL-yts): `fetch_expected_checksum`
returned `None` for missing, malformed, and transport-failed checksum metadata, and
`verify_download` accepted `None` — so a release asset whose integrity could not be
authenticated was installed anyway, with only a verbose-mode skip warning. An attacker
able to suppress or corrupt checksum metadata (an outage, a CDN failure, or an active
attack) could substitute a tampered binary and ddl would install it silently.

## What Changes

- **Secure default (breaking for unverified releases)**: a binary release download is
  installed only when its sha256 is verified against a published checksum. When the
  checksum metadata is missing, malformed, incomplete (asset absent from the manifest),
  or the metadata fetch fails transport-wise, the download is REJECTED with
  `DdlError::ChecksumUnavailable` before any destination mutation.
- **Explicit legacy-unverified transition**: users can authorize an unverified install
  of a release that publishes no usable checksum by setting
  `DDL_ALLOW_UNVERIFIED_RELEASE=1`. This is never automatic, never implied, and never
  applies to a *published* checksum mismatch.
- **No cargo fallback on rejection**: checksum rejection and mismatch are terminal;
  only `NoReleaseBinary` (no asset for the platform) triggers the cargo fallback.
- **Output distinction**: human output and install-result messages distinguish
  `sha256 verified` installs, `rejected` downloads, and explicitly authorized
  `unverified — explicitly authorized via DDL_ALLOW_UNVERIFIED_RELEASE` installs.
  JSON collector envelopes carry a per-tool `verification` field
  (`verified` | `unverified-authorized`).

## Impact

- Affected specs: `bootstrap`
- Affected code: `src/installer.rs`, `src/error.rs`, `src/output.rs`,
  `tests/install_policy.rs`
- Breaking: releases that publish no usable checksum no longer install by default.
  The compatibility transition is the explicit env-var authorization, not a silent
  weakening of the default.
