# Project Context

## Purpose

dulce-de-leche (CLI: `ddl`) is a cross-platform bootstrap and orchestration tool for the charly-vibes ecosystem. It provides a single entry point to install, configure, and manage all charly-vibes CLI tools (wai, dont, ah, pretender, testaruda, fotos-mcp, fabbro) across macOS, Linux, and Windows.

The core value proposition is: **one binary, one command, any platform.** Download a single pre-compiled static binary, run `ddl init`, and get the entire charly-vibes toolset working on any platform — no package manager prerequisites, no Rust toolchain required.

## Tech Stack

- Rust (clap for CLI, miette for diagnostics, genesis-vibes for scaffold/doctor)
- TOML for configuration
- JSON for manifest file
- Subprocess communication with managed tools (no shared library linking)
- Cross-compilation for macos/linux/windows, amd64/arm64

## Project Conventions

### Architecture Patterns

- **Subprocess protocol**: ddl communicates with each tool via subprocess calls (e.g., `wai status --json`). No shared library linking. This decouples release cycles.
- **Platform detection**: ddl detects the OS and architecture at runtime and selects the appropriate installation strategy.
- **Fallback chain**: binary download → cargo install → brew/scoop install. The most reliable path is tried first.
- **Idempotent operations**: every command can be re-run safely. No destructive side effects without confirmation.

### Directory Structure

```
openspec/
├── project.md          # Project context (this file)
├── specs/              # Capability specifications
│   ├── cli-core/       # CLI command structure, flags, output
│   ├── bootstrap/      # Installation, platform detection, init
│   ├── health/         # Status aggregation, doctor
│   └── version-management/  # Version pinning, upgrade, manifest
└── changes/            # Active change proposals
    └── <change-id>/
        ├── proposal.md
        ├── tasks.md
        ├── design.md (optional)
        └── specs/
            └── <capability>/spec.md
```

### Domain Context

#### Core Concepts

- **ddl**: The CLI binary name for dulce-de-leche
- **`.ddl/`**: The configuration directory created by `ddl init`, containing per-tool configs and a shared manifest
- **Manifest** (`.ddl/manifest.json`): Tracks installed tool versions, migration state, and ddl version
- **Platform strategy**: The decision matrix mapping each (OS, arch) pair to the best installation method
- **Managed tool**: Any charly-vibes tool that ddl knows how to install and check
- **Bootstrap**: The first-time setup flow that installs tools and creates `.ddl/`

#### In Scope

| Concern | How |
|---------|-----|
| Cross-platform bootstrap | `ddl init` detects platform, downloads/calls appropriate installer |
| Multi-tool install | `ddl install <tool>` installs a single tool |
| Cross-tool status | `ddl status` aggregates health across all tools via subprocess |
| Cross-tool doctor | `ddl doctor` runs diagnostics for all tools |
| Version pinning | `.ddl/manifest.json` tracks installed versions for reproducibility |
| Global upgrade | `ddl upgrade` updates all tools to latest compatible versions |
| Config directory | `.ddl/` houses per-tool configs (symlinks in Phase 1) |

#### Explicitly Out of Scope

- Package resolver — ddl calls existing package managers, does not reimplement them
- Unified CLI launcher — each tool keeps its own CLI grammar
- Database merge — dont and testaruda keep independent stores (Cozo, SQLite)
- CI/CD integration — each tool handles its own CI

#### Managed Tools

| Tool (binary) | Cargo crate | Homebrew formula | GitHub repo | Genesis consumer? | Status |
|---|---|---|---|---|---|
| `wai` | `wai-cli` | `wai.rb` ✓ | `charly-vibes/wai` | ✓ | Active |
| `dont` | `dont-cli` | `dont.rb` (placeholder) | `charly-vibes/dont` | ✓ | Active |
| `ah` | `espectacular` | `ah.rb` (placeholder) | `charly-vibes/espectacular` | ✓ | Active |
| `pretender` | `pretender` | `pretender.rb` (placeholder) | `charly-vibes/pretender` | ✓ | Active |
| `testaruda` | `testaruda` | — (not in homebrew) | `charly-vibes/testaruda` | ✓ | Active |
| `fotos-mcp` | `fotos-mcp` | `fotos-mcp.rb` ✓ | `charly-vibes/fotos` | ✗ | Active |
| `fabbro` | — (Go) | `fabbro.rb` (placeholder) | `charly-vibes/fabbro` | ✗ (Go) | Active |
| `vampiro` | — (planned) | — | `charly-vibes/vampiro` | planned | In spec |

**Important:** For cargo install, use the **crate name** (e.g., `cargo install espectacular`, NOT `cargo install ah`). For brew install, use the formula name minus `.rb` (e.g., `brew install ah`). This mapping is defined in the bootstrap spec's Design Rationale.

### Important Constraints

- CLI must be fast (<100ms for simple commands)
- Errors must always suggest fixes (self-healing)
- Must work offline (no network required for already-installed tools)
- Must handle partial installations gracefully (some tools may fail to install)
- Must detect placeholder Homebrew formulas and skip them with clear messaging
- Binary must be statically linked for Linux deployment
- All commands must support `--json` for machine-readable output
- Must respect `NO_COLOR` environment variable

### External Dependencies

- Runtime: none (static binary)
- Installation: optionally calls brew, cargo, or scoop
- Communication: subprocess calls to managed tools