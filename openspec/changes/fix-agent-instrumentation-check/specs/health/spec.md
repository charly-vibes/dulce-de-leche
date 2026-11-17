# health (delta)

## ADDED Requirements

### Requirement: Repository Instrumentation Check

The `ddl doctor` repository checks SHALL verify that each tool used by the
repository publishes its own agent-instruction artifact, and SHALL NOT require
any tool to reproduce an artifact that tool has retired.

Each check entry SHALL declare what the tool publishes — a managed block in
`AGENTS.md` or a published artifact path — and SHALL be evaluated only when
the repository uses that tool, defined as: the tool's own directory exists
(`openspec/`, `.wai/`, `.beads/`) or the tool is pinned in
`versions.ddl.toml`.

#### Scenario: Block-publishing tool is missing its block

- **WHEN** the repository uses a tool that owns a managed block (ddl, wai,
  beads) and `AGENTS.md` does not contain that tool's block markers
- **THEN** doctor reports a warning naming the tool and the command that
  writes the block
- **AND** the finding stays a warning; `ddl doctor`'s exit code is unchanged

#### Scenario: Artifact-publishing tool is present

- **WHEN** the repository contains `openspec/config.yaml`
- **THEN** the openspec instrumentation check passes with no warning
- **AND** no managed block is required

#### Scenario: Legacy openspec layout is reported as migration, not failure

- **WHEN** `openspec/project.md` exists and `openspec/config.yaml` does not
- **THEN** doctor reports a warning stating that the layout is legacy, naming
  `openspec update` and the `project.md` → `config.yaml` migration
- **AND** the fix hint does not name `ddl init`

#### Scenario: Hybrid openspec layout

- **WHEN** both `openspec/project.md` and `openspec/config.yaml` exist
- **THEN** the instrumentation check passes on the artifact
- **AND** doctor emits an advisory that `config.yaml` is authoritative and
  `project.md` is stale migration input

#### Scenario: Tool not used by this repository

- **WHEN** a repository has neither the tool's directory nor a pin for it
- **THEN** doctor emits no finding for that tool

#### Scenario: Tool used but not installed

- **WHEN** the repository uses openspec and the `openspec` binary is not
  reachable on PATH
- **THEN** doctor reports the artifact state together with "tool not
  installed"
- **AND** emits no fix hint that requires the missing binary

#### Scenario: A retired block is never demanded

- **WHEN** the repository's `AGENTS.md` contains no `OPENSPEC:START` block
- **THEN** no warning is produced for its absence, whether the repository is
  on the legacy or the artifact openspec layout