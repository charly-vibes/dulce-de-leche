# Spec Delta — health (modified capability)

## ADDED Requirements

### Requirement: Status Command reports project status

The `ddl status` report SHALL include a project-status section beside the tool-health
section, sourced from ddl's status record (`repo-status` capability), so one command
answers both tool health and repo status.

#### Scenario: Record missing

- **WHEN** user runs `ddl status` and `.ddl/status.json` does not exist
- **THEN** the project-status section reports "no status record — run `ddl status
  --refresh`"
- **AND** the tool-health section still renders normally
- **AND** the exit code is governed by tool health only

#### Scenario: Stale record

- **WHEN** user runs `ddl status` and the record's `generated_at` is older than the
  last commit touching any feed's tracked store file (cheap display-time heuristic)
- **THEN** the project-status section shows the record with a "may be stale" flag
- **AND** suggests `ddl status --refresh`
- **AND** the authoritative content-diff staleness verdict is computed only by
  `ddl status --refresh` and the `ddl.repo.status-fresh` doctor check

### Requirement: Doctor Command reports repo checks

The `ddl doctor` report SHALL include repo-status checks (namespace `ddl.repo.*`)
separated from tool diagnostics.

#### Scenario: Repo checks reported separately

- **WHEN** user runs `ddl doctor`
- **THEN** the report separates tool diagnostics from repo checks (`status-fresh`,
  `beads-block`, `brief-drift-gate`, `unmanaged-status-artifacts`)
- **AND** a repo check failure does not change how tool failures are counted
- **AND** `ddl doctor --json` groups repo checks under a `repo` key

#### Scenario: Repo check fixes a stale record

- **WHEN** user runs `ddl doctor --fix` and the status record is stale or missing
- **THEN** the system refreshes the status record via the obtain-and-write ordering
- **AND** reports the refresh as an applied fix

## MODIFIED Requirements

### Requirement: Status Command

The CLI SHALL provide `ddl status` to show a quick overview of all managed tools and
the repository's project status.

#### Scenario: All tools healthy

- **WHEN** user runs `ddl status` and all tools are installed, configured, and healthy
- **THEN** the system displays a summary with each tool's status
- **AND** uses ✓ for healthy, ⚠ for warning, ✗ for failed
- **AND** shows tool versions
- **AND** renders the project-status section from the status record
- **AND** exits with code 0

#### Scenario: Some tools not installed

- **WHEN** user runs `ddl status` and some tools are not installed
- **THEN** the system displays ✗ for missing tools
- **AND** suggests `ddl install <tool>` for each missing tool
- **AND** exits with code 1

#### Scenario: Some tools unhealthy

- **WHEN** user runs `ddl status` and some tools are installed but unhealthy
- **THEN** the system displays ⚠ for each unhealthy tool
- **AND** suggests `ddl doctor` for detailed diagnostics
- **AND** exits with code 1

#### Scenario: Status as JSON

- **WHEN** user runs `ddl status --json`
- **THEN** the system outputs JSON following the format defined in `cli-core/spec.md` (Requirement: JSON Output)
- **AND** the data field contains each tool's name, version, status, and message
- **AND** the data field contains a `project` object with the in-flight summary, tracker counts, and per-feed `observed_at`/`staleness`

#### Scenario: Status reads from subprocess

- **WHEN** user runs `ddl status`
- **THEN** ddl calls each installed tool's status command via subprocess
- **AND** parses the JSON output (preferred) or human output (fallback)
- **AND** includes the tool's own diagnostic output in the ddl report

#### Scenario: Tool subprocess fails

- **WHEN** ddl calls a tool's status command and the subprocess fails
- **THEN** ddl reports the tool as "unreachable"
- **AND** includes the subprocess error output
- **AND** continues checking remaining tools
- **AND** exits with code 1

#### Scenario: No tools installed

- **WHEN** user runs `ddl status` and no tools are installed
- **THEN** the system displays "No charly-vibes tools installed"
- **AND** suggests `ddl init`
- **AND** exits with code 1

#### Scenario: Discovery mode (no manifest)

- **WHEN** user runs `ddl status` and `.ddl/manifest.json` does not exist
- **THEN** the system scans PATH for known tool binaries (wai, dont, ah, pretender, testaruda, vampiro, turu, specodelic, bd, openspec, incitaciones)
- **AND** reports each found tool as "detected (PATH)"
- **AND** suggests `ddl init` to create a manifest
- **AND** exits with code 0 if all tools found, code 1 if some missing

### Requirement: Doctor Command

The CLI SHALL provide `ddl doctor` to run detailed diagnostics on all managed tools and
on the repository's status pipeline.

#### Scenario: Full diagnostics

- **WHEN** user runs `ddl doctor`
- **THEN** the system runs each tool's doctor command via subprocess
- **AND** aggregates the results into a single report
- **AND** shows pass/warn/fail status for each tool
- **AND** includes fix suggestions from each tool's doctor output
- **AND** exits with code 0 if all pass, code 1 if any fail

#### Scenario: Doctor as JSON

- **WHEN** user runs `ddl doctor --json`
- **THEN** the system outputs JSON following the format defined in `cli-core/spec.md`
- **AND** groups results by tool with pass/warn/fail counts
- **AND** groups repo-status checks under a `repo` key

#### Scenario: Doctor fix

- **WHEN** user runs `ddl doctor --fix`
- **THEN** the system runs each tool's doctor with auto-fix enabled
- **AND** reports which fixes were applied and which failed
- **AND** exits with code 0 if all fixes applied, code 1 if any failed

#### Scenario: Tool doctor not available

- **WHEN** ddl runs doctor for a tool that doesn't have a doctor command
- **THEN** ddl reports "doctor not available" for that tool
- **AND** performs a basic check (is the binary in PATH? is the config directory valid?)
- **AND** continues with remaining tools
- **AND** does not change the exit code (basic checks are not failures)
