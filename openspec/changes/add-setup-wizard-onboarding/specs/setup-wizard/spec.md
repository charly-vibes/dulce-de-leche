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
  healthy (or explicitly reporting a tool as unavailable on the platform,
  with no partial-failure ambiguity) and `ddl doctor` repo checks passing
  for the checks the wizard itself writes (managed blocks, gates, pins,
  gitignore) — the same scoping as the DDL-gef pin-contract framing: checks
  that depend on external tool versions are out of the wizard's guarantee

#### Scenario: Wizard failure is resumable

- **WHEN** any step of the wizard fails mid-run
- **THEN** the wizard exits non-zero with a summary naming the exact
  re-run/finish commands, leaving no partial repo state beyond what the
  standard `ddl init` flow would leave

### Requirement: Scripted invocation

The system SHALL support `ddl setup --yes` as a scripted, deterministic run
that accepts defaults at every prompt (used by the TTFIF harness and by
agents), and SHALL refuse the interactive wizard when stdout is not a TTY
and `--yes` is absent, pointing to `ddl init --yes`.

#### Scenario: Agent runs the wizard non-interactively

- **WHEN** `ddl setup --yes` runs with no TTY
- **THEN** the wizard completes with defaults and the same end-state
  guarantees as the interactive flow

#### Scenario: Interactive wizard without a TTY

- **WHEN** `ddl setup` runs without `--yes` and stdout is not a TTY
- **THEN** the command exits non-zero with a message pointing to
  `ddl init --yes`

### Requirement: Pre-flight reports environment gaps

The wizard SHALL detect missing prerequisites and unavailable network
before installing, reporting each as a named, actionable state.

#### Scenario: No network access

- **WHEN** the wizard's pre-flight detects network unavailability
- **THEN** the wizard reports which steps will fail and offers to proceed
  with offline-safe steps only (config without installs), instead of
  failing mid-install

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
