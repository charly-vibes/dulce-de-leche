# Version Management

## Purpose

Define how ddl tracks, pins, and updates tool versions across environments. This ensures reproducibility: the same `ddl init` produces the same toolset on any machine.

## Problem Statement

Without version pinning, `ddl init` on two different days could install different versions of tools. This creates "works on my machine" problems for the toolchain itself. A manifest file ensures that all environments get the same tool versions.

## Design Rationale

### Manifest Format

The manifest (`.ddl/manifest.json`) is the source of truth for which tools are installed, at what versions, and via which source. It is written by `ddl init` and `ddl install`, and read by `ddl status`, `ddl doctor`, and `ddl upgrade`. The manifest is committed to the repository (config changes are reproducible) while data files are gitignored.

### Version Compatibility Matrix

ddl uses a compatibility matrix to prevent installing tool versions with incompatible config conventions. The matrix is **fetched dynamically** from `https://charly-vibes.github.io/ddl/compatibility.json` at runtime. This ensures the matrix is always fresh without requiring a ddl release. An embedded fallback matrix is used when offline.

### Online vs Offline Mode

- **Online**: `ddl version --check` queries the latest available versions of all tools and the compatibility matrix
- **Offline**: `ddl version` (without `--check`) shows installed versions from the manifest only, using the embedded fallback matrix for compatibility checks
- **`ddl upgrade`** requires network access (must query latest versions)
- **`ddl status`** works offline (reads from manifest)

### Binary-Downloaded Tool Upgrades

When a tool was installed via binary download (the preferred path), `ddl upgrade` re-downloads the binary from the tool's GitHub releases. The upgrade fetches the latest release tag, downloads the binary for the current platform, and replaces the existing binary. This is the same mechanism as the initial binary download.

## Scope

### Non-Goals

- Semantic version constraint solving (dependencies between tool versions)
- Rolling back tool versions (use the platform's package manager for this)
- Managing tool updates outside ddl (if a user runs `brew upgrade wai`, ddl detects the version change on next status check)

## Requirements

### Requirement: Manifest File

The CLI SHALL maintain a `.ddl/manifest.json` file tracking installed tool versions.

#### Scenario: Manifest creation

- **WHEN** user runs `ddl init` or `ddl install <tool>`
- **THEN** the system creates or updates `.ddl/manifest.json`
- **AND** records the ddl version, each tool's installed version, installation source, and installation status

#### Scenario: Manifest structure

- **WHEN** `.ddl/manifest.json` is written
- **THEN** it contains:
  - `ddl_version`: the version of ddl that wrote this manifest
  - `migration_state`: the migration phase (e.g., `"phase1"`, `"none"`)
  - `tools`: an object with one entry per tool, each containing `installed` (version string), `source` (e.g., `"brew"`, `"cargo"`, `"binary"`), `status` (`"installed"`, `"pending"`, `"failed"`), and `compatible` (version constraint string)

#### Scenario: Manifest read

- **WHEN** user runs `ddl status` or `ddl doctor`
- **THEN** the system reads `.ddl/manifest.json` to determine which tools are installed
- **AND** compares installed versions against the compatibility matrix

#### Scenario: Manifest missing

- **WHEN** user runs `ddl status` and `.ddl/manifest.json` does not exist
- **THEN** the system scans for tool binaries in PATH
- **AND** warns that the manifest is missing
- **AND** suggests `ddl init` to create it

### Requirement: Version Command

The CLI SHALL provide `ddl version` to show the versions of ddl and all managed tools.

#### Scenario: Offline version display

- **WHEN** user runs `ddl version` (without `--check`)
- **THEN** the system displays the ddl version
- **AND** displays each managed tool's installed version from the manifest
- **AND** does NOT require network access
- **AND** exits with code 0

#### Scenario: Online version check

- **WHEN** user runs `ddl version --check`
- **THEN** the system queries the latest available versions of all tools
- **AND** marks outdated tools with "update available" and the new version
- **AND** requires network access
- **AND** exits with code 0 if all up to date, code 1 if any outdated

#### Scenario: Version as JSON

- **WHEN** user runs `ddl version --json`
- **THEN** the system outputs JSON following the format defined in `cli-core/spec.md`
- **AND** includes ddl version and managed tool versions
- **AND** works offline (reads from manifest)

#### Scenario: Version check fails (no network)

- **WHEN** user runs `ddl version --check` and network is unavailable
- **THEN** the system displays installed versions from manifest
- **AND** reports: "⚠ Network unavailable — showing cached versions"
- **AND** exits with code 0

### Requirement: Upgrade Command

The CLI SHALL provide `ddl upgrade` to update all managed tools to their latest compatible versions.

#### Scenario: Upgrade all tools

- **WHEN** user runs `ddl upgrade`
- **THEN** the system reads `.ddl/manifest.json`
- **AND** fetches the latest compatibility matrix
- **AND** for each tool, calls the appropriate upgrade method:
  - brew-installed: `brew upgrade <formula>`
  - cargo-installed: `cargo install <crate>`
  - binary-downloaded: re-downloads binary from GitHub releases
  - scoop-installed: `scoop update <app>`
- **AND** verifies the new version
- **AND** updates `.ddl/manifest.json`
- **AND** reports which tools were upgraded and which were already up to date

#### Scenario: Upgrade single tool

- **WHEN** user runs `ddl upgrade wai`
- **THEN** the system upgrades only the specified tool using the appropriate upgrade method

#### Scenario: Upgrade binary-downloaded tool

- **WHEN** user runs `ddl upgrade` and a tool was installed via binary download
- **THEN** ddl fetches the latest release tag from the tool's GitHub repo
- **AND** downloads the binary for the current platform
- **AND** replaces the existing binary
- **AND** updates `.ddl/manifest.json`

#### Scenario: No updates available

- **WHEN** user runs `ddl upgrade` and all tools are at their latest versions
- **THEN** the system reports "All tools are up to date"
- **AND** exits with code 0

#### Scenario: Upgrade fails

- **WHEN** an upgrade command fails for a tool
- **THEN** ddl reports the failure with the error output
- **AND** continues upgrading remaining tools
- **AND** exits with code 1 (partial failure)

#### Scenario: Upgrade requires network

- **WHEN** user runs `ddl upgrade` and network is unavailable
- **THEN** the system reports "⚠ Network unavailable — cannot upgrade"
- **AND** suggests `ddl version` to view installed versions
- **AND** exits with code 1

### Requirement: Compatibility Matrix

The CLI SHALL use a compatibility matrix to prevent installing incompatible tool versions.

#### Scenario: Dynamic matrix fetch

- **WHEN** user runs `ddl install`, `ddl upgrade`, or `ddl version --check`
- **THEN** the system fetches the compatibility matrix from `https://charly-vibes.github.io/ddl/compatibility.json`
- **AND** caches it in `.ddl/compatibility-cache.json` for offline use
- **AND** falls back to the embedded matrix if the fetch fails

#### Scenario: Compatible version

- **WHEN** user runs `ddl install wai` and the latest wai version is within the compatible range
- **THEN** installation proceeds normally

#### Scenario: Incompatible version

- **WHEN** user runs `ddl install wai` and the latest wai version is outside the compatible range
- **THEN** the system refuses to install
- **AND** displays a message: "wai version X.Y.Z is not compatible with this version of ddl. Please upgrade ddl first."
- **AND** suggests `ddl upgrade` (to upgrade ddl itself)

#### Scenario: Embedded fallback matrix

- **WHEN** the dynamic matrix fetch fails (no network, server down)
- **THEN** the system uses the embedded fallback matrix shipped with the ddl binary
- **AND** reports: "⚠ Using cached compatibility matrix — some versions may be outdated"
- **AND** proceeds with installation

#### Scenario: Compatibility matrix structure

- **WHEN** the compatibility matrix is fetched or read from cache
- **THEN** it contains version constraints for each managed tool (e.g., `"wai": ">=2026.3.0"`)
- **AND** is updated independently of ddl releases (with each tool release)