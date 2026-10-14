# Change: Replay tool installations from exact versions and artifact identities

## Why

Security evaluation 2026-10-09, finding SEC-04 (DDL-5bs): installs float.
Binary installs resolve `releases/latest` at install time, `cargo install` has
no `--version`/`--locked`, and npm installs are unversioned (`@latest`). The
installed-state manifest records no artifact hash. Two machines installing the
same tool on the same day can therefore receive different bytes, and no
recorded identity exists to reproduce (replay) a known-good installation.

## What Changes

- **Install lock, distinct from the inventory**: a new `.ddl/lock.json` holds
  one `LockEntry` per tool — exact version, resolved artifact URL, sha256
  digest, and the day the identity was recorded. The manifest
  (`.ddl/manifest.json`) remains the *inventory* of what is installed; the
  lock is the *identity record* of what was verified. They stay separate files
  and separate schemas (manifest gains no new fields; old manifests load
  unchanged).
- **Locked replay**: replaying an installation resolves the artifact purely
  from the lock — the replay path performs zero latest-version resolution (it
  has no latest input at all). Two locked replays select identical versions,
  URLs and digests even when `releases/latest` changes underneath.
- **Replay command construction**: Cargo replay uses the locked version and
  `--locked` dependencies (`cargo install <crate> --version <v> --locked`);
  npm/npx replay uses the selected package version (`<package>@<version>`),
  never `@latest`.
- **Explicit upgrades**: upgrades resolve the latest release *explicitly*, show
  the resolved change (old → new version, URL, digest), and update the lock
  atomically (staged + fsynced + rename, same pattern as the manifest).
- **Release-age policy**: a configurable minimum release age (supply-chain
  cool-down). Upgrades to releases younger than the configured age are
  deferred; time is an explicit input (`today_epoch_day`,
  `published_epoch_day`) so tests are deterministic fixtures, never wall-clock.
- **Offline/error semantics**: replaying a tool with no recorded identity
  fails with a diagnostic that names the tool and instructs recording an
  identity via `ddl install` — it never falls back to floating latest.
- **No new dependencies**.

## Impact

- Affected specs: `bootstrap`
- Affected code: `src/installer.rs` (replay/upgrade policy, command
  construction), `src/manifest.rs` (`InstallLock`/`LockEntry` + atomic save),
  `tests/install_policy.rs` (fixtures in a clearly separated `replay_lock`
  module shared with DDL-wof/DDL-yts/DDL-hsn slices)
- Breaking: none for install behavior by itself — this slice lands the lock
  schema and pure policy functions; wiring the install path to consult the
  lock is the follow-up slice per the SEC-04 sequencing.
- Lock schema, migration (absent/legacy lock file → empty lock), and
  offline/error semantics are specified in `design.md` and the bootstrap delta.
