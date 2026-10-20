# setup-wizard (delta)

## ADDED Requirements

### Requirement: Guided setup entry point

The system SHALL provide a `ddl setup` command that walks a first-time user
from zero to a verified-initialized repo without requiring prior knowledge
of the fleet's command surface.

#### Scenario: New user completes guided setup

- **WHEN** a user runs `ddl setup` in an empty repository on a supported
  platform with prerequisites present
- **THEN** the wizard guides them through install, optional migration, tool
  selection, gates wiring, and verification
- **AND** the process ends with `ddl status` reporting every selected tool
  healthy and `ddl doctor` repo checks passing

#### Scenario: Wizard failure is resumable

- **WHEN** any step of the wizard fails mid-run
- **THEN** the wizard exits non-zero with a summary naming the exact
  re-run/finish commands, leaving no partial repo state beyond what the
  standard `ddl init` flow would leave

### Requirement: Non-interactive paths are unaffected

The system SHALL keep `ddl init` (including `--yes`, `--no-install`,
`--gates`, JSON output) behavior unchanged when the setup wizard is added.

#### Scenario: CI non-interactive init still works byte-for-byte

- **WHEN** `ddl init --yes` runs in a CI pipeline after the wizard ships
- **THEN** its behavior, exit codes, and output shape are identical to the
  pre-wizard flow

### Requirement: Measurement-gated adoption

The setup wizard SHALL NOT be enabled in releases until the measured TTFIF
comparison (design.md decision gate) shows it is at least as fast as the
baseline flow and requires fewer first-user decisions.

#### Scenario: Measurement fails the gate

- **WHEN** captured runs show median wizard TTFIF greater than median
  baseline TTFIF
- **THEN** the wizard stays parked and the comparison is recorded in
  `baseline.org` with the runs that failed the gate
