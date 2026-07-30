# .ddl/ Directory

## Purpose

Define the structure, conventions, and lifecycle of the `.ddl/` directory — the single configuration directory for all charly-vibes tools managed by ddl.

## Problem Statement

Each charly-vibes tool has its own config directory at the repo root (`.wai/`, `.dont/`, `.espectacular/`, `.testaruda/`, `.pretender.toml`). This creates root pollution and makes it unclear which files belong to which tool. The `.ddl/` directory consolidates all tool configs under one roof, with a clear convention for what is committed (config) and what is ignored (data).

## Design Rationale

### Phase 1: Symlink Farm

Phase 1 uses symlinks: `.ddl/wai/ -> .wai/`, `.ddl/dont/ -> .dont/`, etc. The original tool directories remain at the root as symlinks pointing into `.ddl/`. This requires zero tool patches — each tool reads from its original path, unaware that it's a symlink.

**Limitation:** The root is not truly clean until Phase 2 (native config proxy), but the `.ddl/` directory provides a single place to look for all tool configs.

### Phase 2: Native Config Proxy (future)

Each tool (via genesis) learns to check `.ddl/<tool>/` before its legacy location. At that point, the legacy directories can be removed entirely.

### File Ownership

- `.ddl/manifest.json`: Machine-written, machine-read. Committed to repo for reproducibility.
- `.ddl/config.toml`: ddl's own config. Human-editable. Committed.
- `.ddl/<tool>/config.toml`: Per-tool config (symlinked to original location in Phase 1). Committed.
- `.ddl/<tool>/*.db`: Tool data stores (Cozo, SQLite). Gitignored.

### Discovery Rules

ddl discovers `.ddl/` by walking up from CWD (same as git). The nearest ancestor `.ddl/` wins. This allows nested projects (monorepo) to have their own `.ddl/`.

## Scope

### Non-Goals

- Merging tool databases (each tool keeps its own Cozo/SQLite store)
- Providing a UI for browsing `.ddl/` contents
- Auto-migration from legacy configs (Phase 1 symlinks are created by `ddl migrate`, not automatically)

## Requirements

### Requirement: Directory Layout

The `.ddl/` directory SHALL follow a standard layout.

#### Scenario: Standard layout after init

- **WHEN** user runs `ddl init` or `ddl init --yes`
- **THEN** the system creates `.ddl/` with the following structure:
  - `.ddl/manifest.json` — version manifest
  - `.ddl/config.toml` — ddl's own config
  - `.ddl/wai/` — wai config directory
  - `.ddl/dont/` — dont config directory
  - `.ddl/ah/` — espectacular config directory
  - `.ddl/pretender.toml` — pretender config file
  - `.ddl/testaruda/` — testaruda config directory
  - `.ddl/fabbro/` — fabbro config directory

#### Scenario: Per-tool subdirectory

- **WHEN** a tool is installed via `ddl install <tool>`
- **THEN** the system creates the corresponding subdirectory under `.ddl/<tool>/`
- **AND** the subdirectory is a symlink to the tool's legacy config directory in Phase 1

### Requirement: Phase 1 Symlink Convention

The CLI SHALL use symlinks to preserve backward compatibility with existing tool config paths.

#### Scenario: Symlink creation

- **WHEN** `ddl migrate` runs
- **THEN** the system moves `.wai/` contents to `.ddl/wai/`
- **AND** creates a symlink `.wai/` -> `.ddl/wai/`
- **AND** repeats for each installed tool

#### Scenario: Symlink on Windows

- **WHEN** ddl runs on Windows
- **THEN** the system creates directory junctions instead of symlinks (symlinks require admin on Windows)
- **AND** documents this limitation in `ddl doctor`

#### Scenario: Broken symlink detection

- **WHEN** user runs `ddl doctor` and a symlink target is missing
- **THEN** the system reports the broken symlink
- **AND** suggests `ddl migrate --undo` to restore the original layout

### Requirement: Gitignore Convention

The CLI SHALL manage `.gitignore` entries for `.ddl/` data files.

#### Scenario: Gitignore creation

- **WHEN** user runs `ddl init`
- **THEN** the system prompts: "Add `.gitignore` entries for `.ddl/` data files? [Y/n]"
- **AND** if confirmed, adds the following entries to `.gitignore`:
  ```
  # dulce-de-leche — data files (do not commit)
  .ddl/**/*.db
  .ddl/**/store/
  .ddl/install-log.json
  .ddl/doctor-cache.json
  ```

#### Scenario: Gitignore skip

- **WHEN** user runs `ddl init --yes`
- **THEN** the system does NOT modify `.gitignore`
- **AND** documents in the output: "Skipped .gitignore — run `ddl init` interactively to configure"

### Requirement: Manifest File

The CLI SHALL maintain a `.ddl/manifest.json` file tracking installed tool versions.

#### Scenario: Manifest structure

- **WHEN** `.ddl/manifest.json` is written
- **THEN** it contains:
  - `ddl_version`: the version of ddl that wrote this manifest
  - `migration_state`: `"phase1"` or `"phase2"` or `"none"`
  - `tools`: an object with one entry per tool, each containing:
    - `installed`: version string (e.g., `"2026.5.1"`)
    - `source`: installation source (e.g., `"brew"`, `"cargo"`, `"binary"`)
    - `status`: `"installed"`, `"pending"`, or `"failed"` (for retry support)
    - `compatible`: version constraint string (e.g., `">=2026.3.0"`)

#### Scenario: Manifest creation

- **WHEN** user runs `ddl init` or `ddl install <tool>`
- **THEN** the system creates or updates `.ddl/manifest.json`

#### Scenario: Manifest read

- **WHEN** user runs `ddl status` or `ddl doctor`
- **THEN** the system reads `.ddl/manifest.json` to determine which tools are installed
- **AND** compares installed versions against the compatibility matrix

#### Scenario: Manifest missing

- **WHEN** user runs `ddl status` and `.ddl/manifest.json` does not exist
- **THEN** the system scans for tool binaries in PATH
- **AND** warns that the manifest is missing
- **AND** suggests `ddl init` to create it

#### Scenario: Per-tool status tracking

- **WHEN** a tool installation fails
- **THEN** the manifest records `"status": "failed"` for that tool
- **AND** a subsequent `ddl init` or `ddl install` retries failed tools

### Requirement: File Locking

The CLI SHALL use file locking when writing to `.ddl/manifest.json` to prevent corruption from concurrent processes.

#### Scenario: Concurrent write

- **WHEN** two ddl processes attempt to write to manifest.json simultaneously
- **THEN** the second process waits for the first to release the lock
- **AND** both writes complete without corruption

#### Scenario: Lock contention

- **WHEN** a ddl process cannot acquire the lock within a timeout
- **THEN** it reports a warning: "Another ddl process is running. Retrying..."
- **AND** retries up to 3 times before failing