# Spec Delta: bootstrap — replay installations from exact versions and artifact identities

## ADDED Requirements

### Requirement: Install lock records artifact identity

ddl SHALL maintain an install lock (`.ddl/lock.json`), distinct from the
install inventory manifest, recording for each installed tool the exact
version, the concrete artifact URL, the sha256 digest of the verified bytes,
and the day the identity was recorded.

#### Scenario: Verified install records an identity

- **WHEN** an installation completes with a verified artifact
- **THEN** the lock records the tool's exact version, artifact URL, digest,
  and recording day
- **AND** the manifest inventory remains a separate file with its existing
  schema

#### Scenario: Legacy state migrates to an empty lock

- **WHEN** no lock file exists
- **THEN** the lock loads as empty and every replay fails with the
  missing-identity diagnostic until a verified install records an identity

### Requirement: Locked replay performs zero latest-version resolution

Replaying an installation SHALL resolve the artifact purely from the recorded
lock identity. The replay path SHALL NOT consult latest-version resolution,
and two locked replays SHALL select identical versions, source URLs and
digests even when the upstream latest release changes.

#### Scenario: Replay is stable when latest changes

- **WHEN** a tool is replayed from the lock and the upstream
  `releases/latest` has moved to a newer release
- **THEN** the replay installs the recorded version from the recorded URL and
  verifies the bytes against the recorded digest

#### Scenario: Replay without a recorded identity fails offline

- **WHEN** the lock has no entry for the requested tool
- **THEN** the replay fails with a diagnostic naming the tool and instructing
  `ddl install <tool>` to record an identity
- **AND** the replay does not fall back to latest-version resolution

### Requirement: Replay commands are version-pinned

Replay command construction SHALL pin exact versions: Cargo replay uses
`cargo install <crate> --version <locked> --locked`, and npm/npx replay uses
`<package>@<locked-version>`.

#### Scenario: Cargo replay pins version and locks dependencies

- **WHEN** a Cargo-installed tool is replayed from the lock
- **THEN** the command carries `--version <locked>` and `--locked`, and never
  a floating or latest reference

#### Scenario: npm replay pins the package version

- **WHEN** an npm-installed tool is replayed from the lock
- **THEN** the command installs `<package>@<locked-version>` and never
  `@latest`

### Requirement: Explicit upgrades resolve and persist atomically

Explicit upgrades SHALL resolve the latest release explicitly, present the
resolved change (old version/URL/digest → new version/URL/digest), and update
the lock by atomic staged replace. A configurable release-age policy SHALL
defer upgrades to releases younger than the configured minimum age, evaluated
against deterministic time inputs.

#### Scenario: Upgrade shows resolved changes and updates the lock atomically

- **WHEN** an explicit upgrade resolves a new release that passes the
  release-age policy
- **THEN** the resolved change (old and new version, URL, digest) is presented
- **AND** the lock update is an atomic replace: a crash leaves either the old
  or the new lock, never a partial file

#### Scenario: Release-age policy defers fresh releases deterministically

- **WHEN** the candidate release's age (today − publication day) is below the
  configured minimum age
- **THEN** the upgrade is deferred with a reason naming the age shortfall
- **AND** the lock is not modified
