# Bootstrap

## Purpose

Define how ddl installs, initializes, and configures the charly-vibes toolset across different platforms. This is the core value proposition of ddl: one binary, one command, any platform.

## Problem Statement

Installing the charly-vibes toolset currently requires: (1) knowing which tools exist, (2) installing each via a platform-specific package manager, (3) running each tool's init command. This friction increases with each new tool and varies by platform. A single bootstrap command eliminates this friction.

## Design Rationale

### Platform Strategy

ddl detects the OS and architecture at runtime and selects the best installation method for each tool. The fallback chain is: binary download → cargo install → brew/scoop install.

**Supported platforms:** macos-arm64, macos-amd64, linux-arm64, linux-amd64, windows-amd64. On these platforms, ddl guarantees binary availability as the primary path — no prerequisites beyond the binary itself. Unsupported platforms (32-bit, FreeBSD) must use cargo install.

### Tool Name Mapping

Each managed tool has different names across distribution channels. ddl maintains this mapping:

| Tool (binary name) | Cargo crate | Homebrew formula | GitHub repo |
|---|---|---|---|
| `wai` | `wai-cli` | `wai.rb` | `charly-vibes/wai` |
| `dont` | `dont-cli` | `dont.rb` | `charly-vibes/dont` |
| `ah` | `espectacular` | `ah.rb` | `charly-vibes/espectacular` |
| `pretender` | `pretender` | `pretender.rb` | `charly-vibes/pretender` |
| `testaruda` | `testaruda` | — (not in homebrew) | `charly-vibes/testaruda` |
| `fotos-mcp` | `fotos-mcp` | `fotos-mcp.rb` | `charly-vibes/fotos` |
| `fabbro` | — (Go tool) | `fabbro.rb` | `charly-vibes/fabbro` |

For cargo install, use the crate name (e.g., `cargo install espectacular`). For brew install, use the formula name minus `.rb` (e.g., `brew install ah`).

### Prerequisites

Each installation method has prerequisites:

| Method | Prerequisites | Supported platforms |
|--------|--------------|-------------------|
| Binary download | `curl` or `wget`, internet access | All (preferred) |
| Cargo install | `rustc` + `cargo` | All (fallback) |
| Brew install | `brew` | macOS, Linux (with brew) |
| Scoop install | `scoop` | Windows |

The binary download path is the only truly "no prerequisites" path (aside from `curl`/`wget`, which are pre-installed on essentially all systems). On supported platforms, ddl distributes binaries for every release, so binary download is always available.

### First-Run Flow

`ddl init` is the primary entry point. It presents an interactive checklist of available tools, installs the selected ones, runs their init commands, and creates the `.ddl/` directory. In CI environments, `ddl init --yes` runs non-interactively with all tools.

### Placeholder Detection

Some Homebrew formulas (ah, dont, pretender, fabbro) have placeholder versions (0.0.0 with fake SHA256). ddl MUST detect these and skip brew install with a clear message, falling back to cargo install or binary download.

### Network Failure Handling

If a tool installation fails (network error, 404, timeout), ddl records the tool's status as `"failed"` in `.ddl/manifest.json` and continues with remaining tools. A subsequent `ddl init` or `ddl install` retries failed tools. This prevents half-installed states from blocking the user.

## Scope

### Non-Goals

- Installing tools outside the charly-vibes ecosystem
- Resolving version conflicts between tools (each tool manages its own dependencies)
- Installing the Rust toolchain (cargo install is a fallback, not a primary path — ddl does not install rustc)

## Requirements

### Requirement: Platform Detection

The CLI SHALL detect the operating system and CPU architecture at runtime.

#### Scenario: macOS (ARM)

- **WHEN** ddl runs on macOS with Apple Silicon
- **THEN** the system detects `macos-arm64` as the platform
- **AND** prefers Homebrew for installation when available

#### Scenario: macOS (Intel)

- **WHEN** ddl runs on macOS with Intel processor
- **THEN** the system detects `macos-amd64` as the platform
- **AND** prefers Homebrew for installation when available

#### Scenario: Linux (ARM)

- **WHEN** ddl runs on Linux with ARM64 processor
- **THEN** the system detects `linux-arm64` as the platform
- **AND** prefers binary download (primary path) or cargo install (fallback)

#### Scenario: Linux (Intel)

- **WHEN** ddl runs on Linux with AMD64/Intel processor
- **THEN** the system detects `linux-amd64` as the platform
- **AND** prefers binary download (primary path) or cargo install (fallback)

#### Scenario: Windows

- **WHEN** ddl runs on Windows
- **THEN** the system detects `windows-amd64` as the platform
- **AND** prefers Scoop for installation when available
- **AND** falls back to binary download when Scoop is not available
- **AND** binary downloads use `.exe` extension and are placed in `%LOCALAPPDATA%\ddl\bin\`

#### Scenario: Unsupported platform

- **WHEN** ddl runs on an unsupported platform (e.g., 32-bit, FreeBSD)
- **THEN** the system displays a diagnostic error listing supported platforms
- **AND** exits with code 2

### Requirement: Binary Distribution

The CLI SHALL be distributed as a pre-compiled static binary for each supported platform.

#### Scenario: Release assets

- **WHEN** a new version of ddl is released
- **THEN** the release includes binaries for: macos-arm64, macos-amd64, linux-arm64, linux-amd64, windows-amd64
- **AND** each binary is statically linked (no runtime dependencies)

#### Scenario: Binary download

- **WHEN** user downloads a ddl binary
- **THEN** the binary runs without any installation step (no package manager, no toolchain)
- **AND** the binary is self-contained (includes all logic)

### Requirement: Init Command

The CLI SHALL provide `ddl init` as the primary bootstrap command.

#### Scenario: Interactive init (default)

- **WHEN** user runs `ddl init` without flags
- **THEN** the system detects the platform
- **AND** checks prerequisites (curl/wget, brew, cargo, scoop as applicable)
- **AND** presents an interactive checklist of available tools with descriptions
- **AND** prompts the user to select which tools to install (default: all)
- **AND** installs each selected tool via the platform-appropriate method
- **AND** runs each installed tool's init command (e.g., `wai init`, `dont prime`)
- **AND** creates `.ddl/` directory structure
- **AND** writes `.ddl/manifest.json` with installed versions (status: `"installed"` or `"failed"`)
- **AND** optionally adds `.gitignore` entries for `.ddl/` data files

#### Scenario: Non-interactive init (CI)

- **WHEN** user runs `ddl init --yes`
- **THEN** the system proceeds without any prompts
- **AND** installs all available tools
- **AND** fails with a clear error if all tools fail; partial failure reports which succeeded

#### Scenario: Selective install

- **WHEN** user runs `ddl init --tools wai,dont`
- **THEN** the system only installs and configures the specified tools (wai and dont)
- **AND** skips all other tools

#### Scenario: Already initialized

- **WHEN** user runs `ddl init` in a directory that already has `.ddl/`
- **THEN** the system checks `.ddl/manifest.json` for installed tools
- **AND** prompts to install any missing tools
- **AND** retries any tools with `"status": "failed"` in the manifest
- **AND** skips tools that are already installed and configured

#### Scenario: Platform-appropriate installer

- **WHEN** ddl needs to install a tool on macOS
- **THEN** it prefers `brew install <formula>` when the formula has a real release
- **AND** falls back to `cargo install <crate>` when the formula is a placeholder
- **AND** falls back to binary download when neither brew nor cargo is available

#### Scenario: Placeholder formula detection

- **WHEN** ddl detects that a Homebrew formula has version `0.0.0` or fake SHA256
- **THEN** it skips brew install for that tool
- **AND** displays a message: "⚠ <tool> Homebrew formula not yet published — using cargo install instead"
- **AND** falls back to cargo install or binary download

#### Scenario: Prerequisites check

- **WHEN** user runs `ddl init`
- **THEN** the system checks which prerequisites are needed based on the installation plan
- **AND** reports which prerequisites are missing with install guidance
- **AND** does not proceed with installation until prerequisites are met (unless `--yes` is set)
- **AND** prerequisite checks include: `curl` or `wget` (for binary download), `rustc` (for cargo install), `brew` (for brew install), `scoop` (for scoop install)

#### Scenario: Network failure during install

- **WHEN** a tool installation fails due to network error, 404, or timeout
- **THEN** ddl records `"status": "failed"` for that tool in the manifest
- **AND** reports the failure with the error message
- **AND** continues with remaining tools
- **AND** exits with code 1 (partial failure)
- **AND** a subsequent `ddl init` or `ddl install` retries failed tools

#### Scenario: Binary download returns 404

- **WHEN** a binary download URL returns 404 (release not published yet)
- **THEN** ddl does NOT fall back to cargo install
- **AND** reports: "⚠ <tool> binary not yet available for this platform. Try `cargo install <crate>` manually."
- **AND** exits with code 1

### Requirement: Install Command

The CLI SHALL provide `ddl install <tool>` to install a single tool.

#### Scenario: Install known tool

- **WHEN** user runs `ddl install wai`
- **THEN** the system detects the platform
- **AND** looks up the tool in the name mapping table (crate name, formula name)
- **AND** installs via the platform-appropriate method
- **AND** updates `.ddl/manifest.json`

#### Scenario: Install unknown tool

- **WHEN** user runs `ddl install unknown-tool`
- **THEN** the system displays a "did you mean" suggestion if a similar tool name exists
- **AND** exits with code 2

#### Scenario: Reinstall already installed tool

- **WHEN** user runs `ddl install wai` and wai is already installed
- **THEN** the system checks the version
- **AND** if up to date, reports "wai is already up to date"
- **AND** if outdated, suggests `ddl upgrade` or offers to reinstall

#### Scenario: Windows without Scoop

- **WHEN** user runs `ddl install wai` on Windows and Scoop is not installed
- **THEN** ddl reports: "⚠ Scoop not found. Use binary download or install Scoop."
- **AND** downloads the binary directly from GitHub releases
- **AND** places the `.exe` in `%LOCALAPPDATA%\ddl\bin\`
- **AND** suggests adding that path to `%PATH%` if not already present

### Requirement: Init Subcommand

The CLI SHALL run each managed tool's init command after installation.

#### Scenario: wai init

- **WHEN** ddl installs wai via `ddl init` or `ddl install wai`
- **THEN** it runs `wai init` after installation
- **AND** reports success or failure

#### Scenario: dont prime

- **WHEN** ddl installs dont via `ddl init` or `ddl install dont`
- **THEN** it runs `dont prime --plain` after installation
- **AND** reports success or failure

#### Scenario: ah init

- **WHEN** ddl installs ah via `ddl init` or `ddl install ah`
- **THEN** it runs `ah init` after installation
- **AND** reports success or failure

#### Scenario: Tool init fails

- **WHEN** a tool's init command fails
- **THEN** ddl reports the failure with the tool's error output
- **AND** continues with remaining tools
- **AND** exits with code 1 (partial failure)