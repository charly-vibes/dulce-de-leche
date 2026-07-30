# Health

## Purpose

Define how ddl aggregates and reports the health status of all managed charly-vibes tools. This includes `ddl status` (quick overview) and `ddl doctor` (detailed diagnostics).

## Problem Statement

Each charly-vibes tool has its own status and diagnostic commands (`wai status`, `wai doctor`, `dont doctor`, `ah doctor`, `pretender check`, `testaruda doctor`). To check the health of the entire toolset, a user must run 6+ commands. ddl provides a single command that aggregates all tool health into one view.

## Design Rationale

### Subprocess Communication

ddl communicates with each tool via subprocess calls, not shared library linking. This decouples ddl's release cycle from each tool's release cycle. ddl calls `wai status --json`, `dont status --json`, etc. and parses the JSON output. If a tool does not support `--json`, ddl falls back to parsing the human-readable output (degraded, but functional).

### Status vs Doctor

- `ddl status`: quick check — is the tool installed? is it configured? is it working? (<500ms)
- `ddl doctor`: detailed diagnostics — full health check with fix suggestions (<2s)

### Discovery Mode

When no `.ddl/manifest.json` exists, ddl scans PATH for known tool binaries. This allows `ddl status` to work even before `ddl init` is run (e.g., on a machine where tools were installed manually).

## Scope

### Non-Goals

- Modifying tool state (read-only operations)
- Running tool-specific commands (e.g., `ddl dont conclude` — use `dont conclude` directly)
- Diagnosing tool-specific issues beyond what the tool reports

## Requirements

### Requirement: Status Command

The CLI SHALL provide `ddl status` to show a quick overview of all managed tools.

#### Scenario: All tools healthy

- **WHEN** user runs `ddl status` and all tools are installed, configured, and healthy
- **THEN** the system displays a summary with each tool's status
- **AND** uses ✓ for healthy, ⚠ for warning, ✗ for failed
- **AND** shows tool versions
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
- **THEN** the system scans PATH for known tool binaries (wai, dont, ah, pretender, testaruda, fotos-mcp, fabbro)
- **AND** reports each found tool as "detected (PATH)"
- **AND** suggests `ddl init` to create a manifest
- **AND** exits with code 0 if all tools found, code 1 if some missing

### Requirement: Doctor Command

The CLI SHALL provide `ddl doctor` to run detailed diagnostics on all managed tools.

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