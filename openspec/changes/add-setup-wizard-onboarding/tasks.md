# Tasks: add-setup-wizard-onboarding

## 1. Measurement (blocks implementation)

- [x] 1.1 Define TTFIF metric + decision gate (design.md)
- [x] 1.2 Implement `scripts/measure-ttfif.sh` clean-room harness
- [x] 1.3 Capture baseline runs (current flow) into `baseline.org` — config-only
      5.73s; full flow CANNOT reach a healthy fleet: funkalization/wild have no
      platform binaries and no cargo fallback (partial failure). Gate comparison
      must use a defined subset with published binaries, or wait for releases.
- [ ] 1.4 Capture wizard runs — **blocked** on implementation (§2)
- [ ] 1.5 Run the decision gate comparison; record result in `baseline.org`

## 2. Implementation (only if gate passes)

- [ ] 2.1 `src/cli.rs`: add `setup` subcommand (red-first test)
- [ ] 2.2 `src/setup_wizard.rs`: step machine (pre-flight → migrate? → scope →
      delegate to init/install paths → verify)
- [ ] 2.3 Reuse contract: wizard delegates to the same installer/gates code
      paths as `ddl init` (test: shared-path assertion or refactor guard)
- [ ] 2.4 Failure/resume summary parity with init (test)
- [ ] 2.5 Book page `docs/src/setup.md` + llms.txt/llm.txt sync

## 3. Release

- [ ] 3.1 Update managed tool docs if the command surface block changes
- [ ] 3.2 CHANGELOG entry
