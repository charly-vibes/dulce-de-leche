# Bootstrap Delta

## MODIFIED Requirements

### Requirement: Platform Detection

The CLI SHALL detect the operating system and CPU architecture at runtime.

On every supported platform, the preferred installation method is binary download; cargo install is the fallback when a release binary is unavailable and cargo is on PATH. No package manager (brew, scoop) participates in method selection.

#### Scenario: macOS (ARM)

- **WHEN** ddl runs on macOS with Apple Silicon
- **THEN** the system detects `macos-arm64` as the platform
- **AND** prefers binary download (primary path) or cargo install (fallback)

#### Scenario: macOS (Intel)

- **WHEN** ddl runs on macOS with Intel processor
- **THEN** the system detects `macos-amd64` as the platform
- **AND** prefers binary download (primary path) or cargo install (fallback)

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
- **AND** prefers binary download (primary path) or cargo install (fallback)
- **AND** binary downloads use `.exe` extension and are placed in `%LOCALAPPDATA%\ddl\bin\`

#### Scenario: Unsupported platform

- **WHEN** ddl runs on an unsupported platform (e.g., 32-bit, FreeBSD)
- **THEN** the system displays a diagnostic error listing supported platforms
- **AND** exits with code 2

### Requirement: Init Command

The CLI SHALL provide `ddl init` as the primary bootstrap command.

#### Scenario: Interactive init (default)

- **WHEN** user runs `ddl init` without flags
- **THEN** the system detects the platform
- **AND** checks prerequisites (curl/wget for binary download; cargo for the fallback path)
- **AND** presents an interactive checklist of available tools with descriptions
- **AND** prompts the user to select which tools to install (default: all)
- **AND** installs each selected tool via binary download, falling back to cargo install when no release binary is available
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

#### Scenario: Binary-first install with cargo fallback

- **WHEN** ddl needs to install a tool on any platform
- **THEN** it downloads the release binary from the tool's GitHub releases
- **AND** when the release has no binary for the current platform (404 or asset missing)
- **AND** cargo is available on PATH
- **THEN** it falls back to `cargo install <crate>` and records `source: "cargo"` in the manifest

#### Scenario: No release binary and no cargo

- **WHEN** a tool has no release binary for the current platform
- **AND** cargo is not installed
- **THEN** ddl reports an error naming both remedies: installing Rust (https://rustup.rs) to enable cargo install, or downloading the binary manually from the tool's GitHub releases
- **AND** records `"status": "failed"` for that tool

#### Scenario: Prerequisites check

- **WHEN** user runs `ddl init`
- **THEN** the system checks which prerequisites are needed based on the installation plan
- **AND** reports which prerequisites are missing with install guidance
- **AND** does not proceed with installation until prerequisites are met (unless `--yes` is set)
- **AND** prerequisite checks include: `curl` or `wget` (for binary download) and `cargo` (for the fallback path)

#### Scenario: Network failure during install

- **WHEN** a tool installation fails due to network error, 404, or timeout
- **THEN** ddl records `"status": "failed"` for that tool in the manifest
- **AND** reports the failure with the error message
- **AND** continues with remaining tools
- **AND** exits with code 1 (partial failure)
- **AND** a subsequent `ddl init` or `ddl install` retries failed tools

#### Scenario: Binary download returns 404

- **WHEN** a binary download URL returns 404 (release not published yet)
- **AND** cargo is available on PATH
- **THEN** ddl falls back to `cargo install <crate>`
- **AND** reports: "⚠ <tool> binary not yet available for this platform — using cargo install instead"

#### Scenario: Binary download returns 404 without cargo

- **WHEN** a binary download URL returns 404 (release not published yet)
- **AND** cargo is not available on PATH
- **THEN** ddl does NOT attempt any further install method
- **AND** reports: "⚠ <tool> binary not yet available for this platform, and cargo is not installed. Install Rust (https://rustup.rs) or download the binary manually from <repo>/releases."
- **AND** exits with code 1

### Requirement: Install Command

The CLI SHALL provide `ddl install <tool>` to install a single tool.

#### Scenario: Install known tool

- **WHEN** user runs `ddl install wai`
- **THEN** the system detects the platform
- **AND** looks up the tool in the name mapping table (crate name)
- **AND** installs via binary download, falling back to cargo install when no release binary is available
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

#### Scenario: Windows binary placement

- **WHEN** user runs `ddl install wai` on Windows
- **THEN** the release binary (`.exe`) is downloaded from GitHub releases
- **AND** placed in `%LOCALAPPDATA%\ddl\bin\`
- **AND** suggests adding that path to `%PATH%` if not already present
