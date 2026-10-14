# Proposal: surface-channel-propagation

## Why

Security evaluation 2026-10-09 (SEC-07 / DDL-9e7) found that both distribution
channel update steps in the release workflow used `continue-on-error: true`,
so a failed Homebrew tap or Scoop bucket update left the release marked
successful while a channel lagged behind — an invisible incomplete
distribution.

## What Changes

- Remove unconditional `continue-on-error` from the tap/bucket update steps.
- Distinguish **intentionally disabled** channels (missing
  `TAP_GITHUB_TOKEN`) from **failed** channel updates; disabled is a
  reported non-failure outcome, failure is retried and then surfaced.
- Retry failed channel updates idempotently (`MAX_ATTEMPTS=3`): the update
  scripts refuse to write unless the declared version, asset tag, and
  checksum entries correspond, and deterministic re-rendering makes a retry
  a no-op against an already-updated tap/bucket. Retries never re-create the
  GitHub release or re-publish to crates.io.
- Add a `Release health (channel propagation)` step that aggregates channel
  outcomes (`updated` / `unchanged` / `disabled` / `failed`) into the job
  summary and fails the job when any channel failed, producing a visible
  incomplete-distribution result for the release.
- New regression meter: `python3 -m unittest discover -s tests -p
  "test_release_channels*.py"`.

## Impact

- Affected specs: `release-pipeline` (channel propagation health requirement)
- Affected code: `.github/workflows/release.yml`,
  `scripts/update-homebrew.py`, `scripts/update-scoop.py`,
  `tests/test_release_channels.py`
- No change to install behavior, manifest format, or public CLI interfaces.
