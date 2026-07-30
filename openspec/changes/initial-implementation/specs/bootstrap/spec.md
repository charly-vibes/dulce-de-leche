## ADDED Requirements
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
- **WHEN** ddl runs on an unsupported platform
- **THEN** the system displays a diagnostic error listing supported platforms
- **AND** exits with code 2

### Requirement: Binary Distribution
The CLI SHALL be distributed as a pre-compiled static binary for each supported platform.

#### Scenario: Release assets
- **WHEN** a new version of ddl is released
- **THEN** the release includes binaries for: macos-arm64, macos-amd64, linux-arm64, linux-amd64, windows-amd64
- **AND** each binary is statically linked

### Requirement: Init Command
The CLI SHALL provide `ddl init` as the primary bootstrap command.

#### Scenario: Interactive init (default)
- **WHEN** user runs `ddl init` without flags
- **THEN** the system presents an interactive checklist of available tools
- **AND** installs each selected tool via the platform-appropriate method
- **AND** runs each installed tool's init command
- **AND** creates `.ddl/` directory structure
- **AND** writes `.ddl/manifest.json` with per-tool status

#### Scenario: Non-interactive init (CI)
- **WHEN** user runs `ddl init --yes`
- **THEN** the system proceeds without any prompts
- **AND** installs all available tools

#### Scenario: Selective install
- **WHEN** user runs `ddl init --tools wai,dont`
- **THEN** the system only installs the specified tools

#### Scenario: Already initialized
- **WHEN** user runs `ddl init` in a directory that already has `.ddl/`
- **THEN** the system retries any tools with `"status": "failed"` in the manifest

#### Scenario: Placeholder formula detection
- **WHEN** ddl detects that a Homebrew formula has version `0.0.0` or fake SHA256
- **THEN** it skips brew install and falls back to cargo install or binary download

#### Scenario: Prerequisites check
- **WHEN** user runs `ddl init`
- **THEN** the system checks which prerequisites are needed based on the installation plan
- **AND** reports which are missing (curl/wget, rustc, brew, scoop)

#### Scenario: Network failure during install
- **WHEN** a tool installation fails due to network error
- **THEN** ddl records `"status": "failed"` in the manifest
- **AND** continues with remaining tools
- **AND** a subsequent `ddl init` retries failed tools

#### Scenario: Binary download returns 404
- **WHEN** a binary download URL returns 404
- **THEN** ddl does NOT fall back to cargo install
- **AND** reports the binary is not yet available for this platform

### Requirement: Install Command
The CLI SHALL provide `ddl install <tool>` to install a single tool.

#### Scenario: Install known tool
- **WHEN** user runs `ddl install wai`
- **THEN** the system looks up the tool in the name mapping table (crate name, formula name)
- **AND** installs via the platform-appropriate method

#### Scenario: Install unknown tool
- **WHEN** user runs `ddl install unknown-tool`
- **THEN** the system displays a "did you mean" suggestion
- **AND** exits with code 2

#### Scenario: Windows without Scoop
- **WHEN** user runs `ddl install wai` on Windows and Scoop is not installed
- **THEN** ddl downloads the binary directly from GitHub releases
- **AND** places the `.exe` in `%LOCALAPPDATA%\ddl\bin\`

### Requirement: Init Subcommand
The CLI SHALL run each managed tool's init command after installation.

#### Scenario: wai init, dont prime, ah init
- **WHEN** ddl installs a tool
- **THEN** it runs the tool's init command after installation
- **AND** reports success or failure

#### Scenario: Tool init fails
- **WHEN** a tool's init command fails
- **THEN** ddl reports the failure and continues with remaining tools
- **AND** exits with code 1 (partial failure)