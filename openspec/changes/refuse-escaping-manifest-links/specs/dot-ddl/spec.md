# Change: Refuse manifest writes redirected by links outside the project

## ADDED Requirements

### Requirement: Manifest writes never follow links

Every write to `.ddl/manifest.json` SHALL open the manifest without following a
link at the manifest path (no-follow open), and SHALL refuse to write when the
open detects a link — regardless of the link's target.

#### Scenario: Escaping manifest symlink

- **WHEN** `.ddl/manifest.json` is a link to a file outside the project and
  `ddl install <tool>` runs
- **THEN** the command fails with an error naming the symlink and SEC-05
- **AND** the outside target file is byte-for-byte unchanged

#### Scenario: Link swapped between check and open

- **WHEN** a regular manifest file is replaced by a link after any path-based
  check has passed
- **THEN** the no-follow open itself fails with the SEC-05 error
- **AND** nothing is written through the link

#### Scenario: Dangling manifest link

- **WHEN** `.ddl/manifest.json` is a dangling link
- **THEN** the write is refused with the SEC-05 error
- **AND** the dangling target path is not created

#### Scenario: In-project manifest link

- **WHEN** `.ddl/manifest.json` is a link to a file inside the project
- **THEN** the write is still refused (a link is mutable state and can be
  swapped between check and write)

### Requirement: Allowed state-root links

Link layouts that stay inside the project SHALL remain supported: `.ddl/` may
be a directory link resolving inside the project root, and post-migration
legacy links (e.g. `.wai` -> `.ddl/wai`) SHALL continue to work end-to-end.

#### Scenario: In-project .ddl directory link

- **WHEN** `.ddl` is a directory link to another directory inside the project
- **THEN** manifest writes proceed normally through the linked directory

#### Scenario: Legacy migration links keep working

- **WHEN** a legacy config location is a link into `.ddl/<tool>/` (the Phase 1
  migration layout) and an install runs
- **THEN** the install succeeds and the legacy link is left untouched

### Requirement: Rejected escaping state-root links

A `.ddl/` directory link resolving outside the project root SHALL be refused
before any manifest write on every supported platform.

#### Scenario: .ddl directory link outside the project

- **WHEN** `.ddl` resolves outside the project root and a manifest write is
  attempted
- **THEN** the write fails with an error naming the linked outside root and
  SEC-05
- **AND** no file outside the project is modified

#### Scenario: Windows directory reparse point outside the project

- **WHEN** on Windows `.ddl` is a reparse point to a directory outside the
  project and a manifest write is attempted
- **THEN** the write is refused with the SEC-05 error
