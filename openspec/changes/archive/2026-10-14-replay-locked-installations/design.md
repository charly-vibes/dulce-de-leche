# Design: replay-locked-installations (DDL-5bs, SEC-04)

## Lock schema (v1)

`InstallLock` — serialized to `.ddl/lock.json`:

```json
{
  "entries": {
    "<tool>": {
      "tool": "wai",
      "version": "2026.10.3",
      "artifact_url": "https://github.com/charly-vibes/wai/releases/download/v2026.10.3/wai-aarch64-apple-darwin.tar.gz",
      "sha256": "<hex digest of the verified artifact>",
      "recorded_epoch_day": 20470
    }
  }
}
```

Field semantics:

- `version` — the exact installed version, as resolved at install time.
- `artifact_url` — the concrete URL the verified bytes were fetched from
  (tag-pinned, never a `latest` redirect).
- `sha256` — digest of the bytes that passed the DDL-yts checksum policy and
  the DDL-wof provenance policy.
- `recorded_epoch_day` — day the identity was recorded (epoch days, UTC). Used
  by the release-age policy as a deterministic time input.

## Inventory vs lock

The manifest (`ToolEntry`) stays the *inventory*: what is installed, its
status, compatibility. The lock is the *identity record*: what exact bytes
were verified. They are separate files with separate schemas so inventory
mutations (status flips, init bookkeeping) can never corrupt artifact
identities, and the lock can be shared/committed independently. The manifest
gains no new fields in this slice; `Manifest::parse` of legacy manifests is
unchanged (migration is a no-op for the inventory).

## Migration

- Lock file absent → `InstallLock::load` returns an empty lock. Every tool
  then fails replay with the missing-identity diagnostic until the next
  verified install records an identity. This is the only migration state:
  there is no legacy lock format to convert (v1 is the first schema).
- Unknown JSON fields in the lock are ignored by serde (forward-compatible);
  missing fields fail parse with a `Serde` error naming the corrupt file — a
  half-written lock can never exist because saves are staged + fsynced +
  renamed.

## Replay path: zero latest resolution

`replay_decision(lock, tool)` takes the lock and the tool name only — there is
no latest-version input in the signature, so the replay path cannot resolve
latest without an API change. It returns the recorded `LockEntry`, which
feeds the existing verification policy unchanged (checksum/provenance still
enforced on the fetched bytes; a replay whose bytes diverge from the recorded
digest is rejected by the existing policy).

Command construction is pure and pinned:

- Cargo: `cargo install <crate> --version <locked> --locked` (exact version,
  locked dependency graph).
- npm: `npm install -g <package>@<locked-version>` (never `@latest`).

## Explicit upgrades and release-age policy

Upgrades are the only path that resolves latest, and they do so explicitly:
the caller resolves a `ReleaseCandidate` (version, artifact URL, digest of
the candidate artifact, `published_epoch_day`) and hands it to
`upgrade_decision`, which returns one of:

- `UpToDate` — the candidate is byte-identical to the recorded identity
  (same version, URL, digest).
- `Deferred { reason }` — the candidate is younger than the configured
  minimum release age (`today_epoch_day - published_epoch_day <
  min_release_age_days`). The lock is NOT updated.
- `Upgrade { from, to }` — the resolved change, old identity → new identity,
  shown to the user; the caller then persists the new lock entry via
  `InstallLock::save`, which is an atomic staged replace (crash leaves either
  the old or the new lock, never a partial one).

The minimum age is a configuration input (days, `u64`); time enters only as
explicit `*_epoch_day` parameters, so fixtures are deterministic and no test
touches the wall clock.

## Offline / error semantics

- Missing lock entry: `Err(InstallFailed)` naming the tool and instructing
  `ddl install <tool>` to record an identity. No network fallback, no latest
  resolution.
- Lock file corrupt (missing required field): `Err(Serde)` — fail loudly
  rather than silently replaying an unverified guess.
- Replay itself requires no network beyond fetching the recorded artifact;
  identity resolution (the networked part that floats) is absent by
  construction.

## Verifier-dependency justification

None needed: no new verifier or sandbox dependency. Replay verification
reuses the existing sha256 (`sha2`) checksum/provenance policy from DDL-yts /
DDL-wof unchanged.
