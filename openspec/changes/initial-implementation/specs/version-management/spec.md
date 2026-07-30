## ADDED Requirements
### Requirement: Manifest File
The CLI SHALL maintain a `.ddl/manifest.json` file tracking installed tool versions.

#### Scenario: Manifest creation
- **WHEN** user runs `ddl init` or `ddl install <tool>`
- **THEN** the system creates or updates `.ddl/manifest.json`

#### Scenario: Manifest missing
- **WHEN** `.ddl/manifest.json` does not exist
- **THEN** the system scans for tool binaries in PATH
- **AND** warns that the manifest is missing

### Requirement: Version Command
The CLI SHALL provide `ddl version` to show versions.

#### Scenario: Offline version display
- **WHEN** user runs `ddl version` (without `--check`)
- **THEN** the system displays installed versions from the manifest
- **AND** does NOT require network access

#### Scenario: Online version check
- **WHEN** user runs `ddl version --check`
- **THEN** the system queries the latest available versions
- **AND** marks outdated tools with "update available"
- **AND** requires network access

#### Scenario: Version as JSON
- **WHEN** user runs `ddl version --json`
- **THEN** the system outputs JSON following the format defined in `cli-core/spec.md`
- **AND** works offline

### Requirement: Upgrade Command
The CLI SHALL provide `ddl upgrade` to update all managed tools.

#### Scenario: Upgrade all tools
- **WHEN** user runs `ddl upgrade`
- **THEN** the system calls the appropriate upgrade method per tool:
  - brew-installed: `brew upgrade`
  - cargo-installed: `cargo install`
  - binary-downloaded: re-download from GitHub releases
  - scoop-installed: `scoop update`
- **AND** updates `.ddl/manifest.json`

#### Scenario: Upgrade single tool
- **WHEN** user runs `ddl upgrade wai`
- **THEN** the system upgrades only the specified tool

#### Scenario: Upgrade requires network
- **WHEN** user runs `ddl upgrade` and network is unavailable
- **THEN** the system reports "Network unavailable — cannot upgrade"
- **AND** exits with code 1

### Requirement: Compatibility Matrix
The CLI SHALL use a compatibility matrix to prevent installing incompatible tool versions.

#### Scenario: Dynamic matrix fetch
- **WHEN** user runs `ddl install`, `ddl upgrade`, or `ddl version --check`
- **THEN** the system fetches the matrix from `https://charly-vibes.github.io/ddl/compatibility.json`
- **AND** caches it locally
- **AND** falls back to embedded matrix if fetch fails

#### Scenario: Compatible version
- **WHEN** a tool version is within the compatible range
- **THEN** installation proceeds normally

#### Scenario: Incompatible version
- **WHEN** a tool version is outside the compatible range
- **THEN** the system refuses to install
- **AND** suggests upgrading ddl first