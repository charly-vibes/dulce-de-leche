# bootstrap (delta)

## ADDED Requirements

### Requirement: Init Leaves Other Tools' Instruction Artifacts Alone

`ddl init` SHALL invoke each tool's own init command and SHALL NOT migrate,
rewrite, or re-create artifacts that those tools own or have retired.

#### Scenario: Init does not run a tool's migration command

- **WHEN** the init cascade runs for a tool whose configuration directory
  already exists
- **THEN** ddl invokes only that tool's declared init command
- **AND** it does not invoke repair or migration subcommands on the tool's
  behalf

#### Scenario: Init does not re-create a retired block

- **WHEN** a repository is missing a managed block that its tool no longer
  publishes
- **THEN** `ddl init` does not write that block
- **AND** the corresponding doctor finding is supplied by the repository
  instrumentation check, which verifies what the tool publishes instead

#### Scenario: Init respects recorded tool state

- **WHEN** `.ddl/manifest.json` records a tool as initialized
- **THEN** the init cascade skips that tool rather than re-running its init
- **AND** a repository without a manifest is initialized once per checkout,
  which is the only case where a tool's init command runs again