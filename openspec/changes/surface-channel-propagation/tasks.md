# Tasks: surface-channel-propagation

## 1. Regression fixtures (red)

- [x] 1.1 Add `tests/test_release_channels.py`: script verification
      (tag/version/checksum correspondence), idempotency, CLI refusal, and
      workflow channel-semantics assertions; demonstrate red.

## 2. Script verification + idempotency (green)

- [x] 2.1 `scripts/update-homebrew.py`: `verify_release_inputs` (tag must
      equal `v{version}`; all referenced platform assets present in
      checksums), pure `build_formula`, module-level CLI guard, exit 2 on
      verification failure without writing.
- [x] 2.2 `scripts/update-scoop.py`: same verification for the
      `windows_amd64` asset, pure `build_manifest`, CLI guard.

## 3. Workflow channel semantics (green)

- [x] 3.1 Remove unconditional `continue-on-error: true` from both channel
      steps; missing token → `disabled` status, real failure →
      `MAX_ATTEMPTS=3` idempotent retry, machine-readable status files.
- [x] 3.2 Add `Release health (channel propagation)` step: job summary
      table of outcomes; job fails when any channel is `failed`.

## 4. Spec + docs

- [x] 4.1 OpenSpec change proposal and `release-pipeline` spec delta (this
      change).
- [ ] 4.2 Post-approval: archive this change and mark PR ready for merge
      review.
