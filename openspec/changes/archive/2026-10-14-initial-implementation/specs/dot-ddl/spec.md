## ADDED Requirements
### Requirement: Directory Layout
The `.ddl/` directory SHALL follow a standard layout.

#### Scenario: Standard layout after init
- **WHEN** user runs `ddl init` or `ddl init --yes`
- **THEN** the system creates `.ddl/` with per-tool subdirectories for each installed tool

#### Scenario: Per-tool subdirectory
- **WHEN** a tool is installed via `ddl install <tool>`
- **THEN** the system creates the corresponding subdirectory under `.ddl/<tool>/`
- **AND** the subdirectory is a symlink to the tool's legacy config directory in Phase 1

### Requirement: Phase 1 Symlink Convention
The CLI SHALL use symlinks to preserve backward compatibility with existing tool config paths.

#### Scenario: Symlink creation
- **WHEN** `ddl migrate` runs
- **THEN** the system moves each tool's config directory to `.ddl/<tool>/`
- **AND** creates a symlink at the original location

#### Scenario: Symlink on Windows
- **WHEN** ddl runs on Windows
- **THEN** the system creates directory junctions instead of symlinks

#### Scenario: Broken symlink detection
- **WHEN** user runs `ddl doctor` and a symlink target is missing
- **THEN** the system reports the broken symlink and suggests `ddl migrate --undo`

### Requirement: Manifest File
The CLI SHALL maintain a `.ddl/manifest.json` file tracking installed tool versions with per-tool status.

#### Scenario: Manifest structure
- **WHEN** `.ddl/manifest.json` is written
- **THEN** it contains `ddl_version`, `migration_state`, and `tools` with per-tool `installed`, `source`, `status`, and `compatible` fields

#### Scenario: Per-tool status tracking
- **WHEN** a tool installation fails
- **THEN** the manifest records `"status": "failed"` for that tool
- **AND** a subsequent `ddl init` or `ddl install` retries failed tools

### Requirement: File Locking
The CLI SHALL use file locking when writing to `.ddl/manifest.json`.

#### Scenario: Concurrent write
- **WHEN** two ddl processes attempt to write to manifest.json simultaneously
- **THEN** the second process waits for the first to release the lock

#### Scenario: Lock contention
- **WHEN** a ddl process cannot acquire the lock within a timeout
- **THEN** it retries up to 3 times before failing

### Requirement: Gitignore Convention
The CLI SHALL manage `.gitignore` entries for `.ddl/` data files.

#### Scenario: Gitignore creation
- **WHEN** user runs `ddl init`
- **THEN** the system prompts to add `.gitignore` entries for `.ddl/**/*.db`, `.ddl/**/store/`, and log files