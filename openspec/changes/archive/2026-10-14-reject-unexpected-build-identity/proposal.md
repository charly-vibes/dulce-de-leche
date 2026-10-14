# Change: Reject release artifacts from an unexpected publisher or build identity

## Why

Security evaluation 2026-10-09, findings SEC-03/06 (DDL-wof): the archive and the
unsigned checksum are fetched from the same release, and the installer performs no
publisher/workflow identity check. A checksum alone authenticates downloaded bytes
against *some* published digest — it says nothing about *who* built and published the
release or under which identity. An attacker able to publish (or re-publish) a release
from an unexpected repository, an unexpected workflow, or a tag other than the one the
asset was fetched from could ship artifacts that pass checksum verification.

## What Changes

- **Release workflow publishes provenance**: `.github/workflows/release.yml` generates
  and uploads `provenance.json` next to `checksums.txt`, binding every published
  artifact's sha256 to the publisher/build identity (`repository`, `workflow`, `tag`).
- **Installer verifies build identity**: `verify_download` chains
  `enforce_provenance_policy` after the DDL-yts checksum policy. The download is
  installed only when the provenance manifest names ddl's trust roots
  (`charly-vibes/dulce-de-leche` and the `Release` workflow) and the tag the asset was
  fetched from, and when the downloaded bytes match the manifest's per-asset digest.
- **Five negative fixtures, all rejections**: altered bytes (digest divergence from the
  manifest), wrong repository, wrong workflow, wrong tag, missing provenance — every
  one rejected before any destination mutation.
- **Identity checks are never bypassed**: `DDL_ALLOW_UNVERIFIED_RELEASE=1` (the
  approved DDL-yts legacy transition) may bridge a *missing* provenance manifest or a
  tag-less archive URL, but never a manifest that *contradicts* the expected identity,
  and never altered bytes.
- **No new dependencies**: verification reuses the existing `reqwest`, `serde_json`,
  and `sha2` crates. See design.md for the verifier-dependency justification.

## Impact

- Affected specs: `bootstrap`
- Affected code: `src/installer.rs`, `.github/workflows/release.yml`,
  `tests/install_policy.rs`
- Breaking: releases that publish no provenance manifest (or publish one naming an
  unexpected identity) no longer install by default. The compatibility transition is
  the same explicit env-var authorization already approved for DDL-yts.
- Trust roots are constants in `src/installer.rs`
  (`EXPECTED_PROVENANCE_REPOSITORY`, `EXPECTED_PROVENANCE_WORKFLOW`); the legacy
  transition and its limits are documented there and in design.md.