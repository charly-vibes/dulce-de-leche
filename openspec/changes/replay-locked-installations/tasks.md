# Tasks: replay-locked-installations

- [x] tests/install_policy.rs — failing DDL-5bs fixtures (red) in a clearly
  separated `replay_lock` module: replay selects recorded identity even when
  latest changes; zero latest-resolution on the replay path; Cargo pins
  version + `--locked`; npm pins `package@version` (never `@latest`);
  explicit upgrade reports resolved changes; deterministic release-age policy;
  lock file round-trip is atomic; missing identity fails with offline
  semantics
- [x] src/manifest.rs — `LockEntry`/`InstallLock` (schema), parse/load with
  absent-file → empty-lock migration, atomic staged-replace save distinct
  from the manifest inventory file
- [x] src/installer.rs — `replay_decision` (no latest input),
  `cargo_replay_args`, `npm_replay_args`, `ReleaseCandidate` +
  `upgrade_decision` with configurable release-age policy and atomic lock
  update payload
- [x] openspec/changes/replay-locked-installations — proposal, design (lock
  schema, migration, offline/error semantics), bootstrap spec delta
- [ ] Follow-up (out of scope, own slice): wire `install_binary`/
  `install_cargo`/`install_npm` to consult the lock on the replay path and
  record identities on fresh installs
