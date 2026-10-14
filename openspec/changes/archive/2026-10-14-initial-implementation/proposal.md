# Change: Initial implementation of dulce-de-leche (ddl)

## Why

The charly-vibes ecosystem has 6+ CLI tools, each with its own install path, config directory, and init command. Setting up the toolset requires running multiple install and init commands, and the experience varies by platform. ddl provides a single binary that bootstraps the entire toolset on any platform — macOS, Linux, or Windows — with no prerequisites.

## What Changes

- **New Rust crate**: `dulce-de-leche` with CLI binary `ddl`
- **`ddl init`**: Interactive bootstrap that installs and configures all charly-vibes tools
- **`ddl install <tool>`**: Install a single tool by name
- **`ddl status`**: Cross-tool health overview via subprocess calls
- **`ddl doctor`**: Detailed diagnostics for all tools
- **`ddl version`**: Show versions of ddl and all managed tools
- **`ddl upgrade`**: Update all tools to latest compatible versions
- **`.ddl/` directory**: Manifest file (`manifest.json`), per-tool config directories
- **Cross-platform binary releases**: Static binaries for macos/linux/windows, amd64/arm64
- **Version compatibility matrix**: Embedded in ddl binary, prevents installing incompatible tool versions
- **Platform detection + fallback chain**: Binary download → cargo install → brew/scoop install

## Impact

- Affected specs: `cli-core`, `bootstrap`, `health`, `version-management`
- Affected code: new crate, all new code
- New repository: `github.com/charly-vibes/dulce-de-leche`
- New Homebrew formula: `dulce-de-leche.rb`
- New Scoop package: `dulce-de-leche.json`
- GitHub Actions: release workflow for cross-platform binary builds
- Non-breaking: does not change any existing tool's behavior