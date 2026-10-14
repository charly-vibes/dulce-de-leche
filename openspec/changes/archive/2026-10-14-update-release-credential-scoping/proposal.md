# Proposal: update-release-credential-scoping

## Why

Security evaluation 2026-10-09 (SEC-06) found that the release workflow granted
`contents: write` workflow-wide, used mutable action tags, published based only
on build-job completion (no test/advisory/secret gates), and CI executed the bd
bootstrap archive without checksum verification. A compromised dependency or
build step could therefore publish releases or move tags with broad credentials.

## What Changes

- Scope `contents: write` and all publication credentials (GITHUB_TOKEN,
  CARGO_REGISTRY_TOKEN, TAP_GITHUB_TOKEN) to the `publish` job only; workflow
  and build/test/security jobs default to `contents: read`.
- Add `test` and `security` jobs in the release workflow that run on the exact
  tagged commit; `publish` needs all build jobs plus these gates, so
  failed/skipped/cancelled prerequisites prevent publication.
- Pin every third-party action to a full reviewed commit SHA.
- Verify the bd bootstrap tarball against a pinned SHA-256 before extraction.
- Add `just advisory` (cargo audit) and `just secret-scan` recipes shared by
  local and CI environments.
- Document credential scopes and verified repository protections (as of
  2026-10-13: no tag rulesets exist; enabling `v*` tag protection is a
  recommended follow-up).

## Impact

- Affected specs: `release-pipeline` (new capability spec)
- Affected code: `.github/workflows/release.yml`, `.github/workflows/ci.yml`,
  `justfile`, `tests/workflow_security.rs` (regression fixtures)
- Non-breaking to public interfaces, output contracts, and installation
  behavior; only CI/CD authorization and gating change.
