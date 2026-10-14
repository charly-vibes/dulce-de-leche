## ADDED Requirements
### Requirement: Status Command
The CLI SHALL provide `ddl status` to show a quick overview of all managed tools.

#### Scenario: All tools healthy
- **WHEN** user runs `ddl status` and all tools are healthy
- **THEN** the system displays a summary with ✓/⚠/✗ per tool
- **AND** exits with code 0

#### Scenario: Some tools missing
- **WHEN** some tools are not installed
- **THEN** the system suggests `ddl install <tool>` for each missing tool
- **AND** exits with code 1

#### Scenario: Status as JSON
- **WHEN** user runs `ddl status --json`
- **THEN** the system outputs JSON following the format defined in `cli-core/spec.md`

#### Scenario: No tools installed
- **WHEN** no tools are installed
- **THEN** the system displays "No charly-vibes tools installed"
- **AND** suggests `ddl init`
- **AND** exits with code 1

#### Scenario: Discovery mode (no manifest)
- **WHEN** `.ddl/manifest.json` does not exist
- **THEN** the system scans PATH for known tool binaries
- **AND** reports found tools as "detected (PATH)"
- **AND** suggests `ddl init` to create a manifest

### Requirement: Doctor Command
The CLI SHALL provide `ddl doctor` to run detailed diagnostics on all managed tools.

#### Scenario: Full diagnostics
- **WHEN** user runs `ddl doctor`
- **THEN** the system aggregates each tool's doctor output into a single report
- **AND** exits with code 0 if all pass, code 1 if any fail

#### Scenario: Doctor as JSON
- **WHEN** user runs `ddl doctor --json`
- **THEN** the system outputs JSON following the format defined in `cli-core/spec.md`

#### Scenario: Doctor fix
- **WHEN** user runs `ddl doctor --fix`
- **THEN** the system runs each tool's doctor with auto-fix enabled

#### Scenario: Tool doctor not available
- **WHEN** a tool doesn't have a doctor command
- **THEN** ddl performs a basic check (binary in PATH, config valid)
- **AND** continues with remaining tools