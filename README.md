> *"Remando en dulce de leche"*
> — Dicho popular

# dulce-de-leche (ddl)

[![tracked with wai](https://img.shields.io/badge/tracked%20with-wai-blue)](https://github.com/charly-vibes/wai)
[![CI](https://github.com/charly-vibes/dulce-de-leche/actions/workflows/ci.yml/badge.svg)](https://github.com/charly-vibes/dulce-de-leche/actions/workflows/ci.yml)
[![Release](https://github.com/charly-vibes/dulce-de-leche/actions/workflows/release.yml/badge.svg)](https://github.com/charly-vibes/dulce-de-leche/actions/workflows/release.yml)
[![Docs](https://github.com/charly-vibes/dulce-de-leche/actions/workflows/docs.yml/badge.svg)](https://github.com/charly-vibes/dulce-de-leche/actions/workflows/docs.yml)
[![crates.io](https://img.shields.io/crates/v/dulce-de-leche.svg)](https://crates.io/crates/dulce-de-leche)
[![Homebrew](https://img.shields.io/badge/brew-charly/dulce--de--leche-blue)](https://github.com/charly-vibes/homebrew-charly)
[![Scoop](https://img.shields.io/badge/scoop-charly/dulce--de--leche-blue)](https://github.com/charly-vibes/scoop-charly)
[![docs.rs](https://img.shields.io/docsrs/dulce-de-leche)](https://docs.rs/dulce-de-leche)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)

**One command to install, configure, and update every charly-vibes tool.**

```
   ddl init     →  installs & configures the whole toolset
   ddl status   →  shows health of all tools at a glance
   ddl migrate  →  moves existing configs under .ddl/
   ddl upgrade  →  updates everything to latest compatible versions
```

Repo root stays clean — everything lives under `.ddl/`.

## Installation

```bash
# macOS (Homebrew)
brew tap charly-vibes/charly
brew install dulce-de-leche

# Linux (binary download)
curl -fsSL https://github.com/charly-vibes/dulce-de-leche/releases/latest/download/ddl-$(uname -s)-$(uname -m).tar.gz | tar xz
sudo mv ddl /usr/local/bin/

# Windows (Scoop)
scoop bucket add charly https://github.com/charly-vibes/scoop-charly.git
scoop install dulce-de-leche

# Any platform (Cargo)
cargo install dulce-de-leche
```

## Quick start

```bash
# Bootstrap the whole toolset
ddl init

# Check the ecosystem
ddl status

# Update everything
ddl upgrade

# See full documentation
ddl --help
```

## The problem

The charly-vibes ecosystem has **6 active Rust CLI tools** (wai, dont, ah/espectacular, pretender, testaruda, fotos-mcp) plus fabbro (Go), and vampiro on the way. Each tool has:

- Its own config file or directory: `.wai/`, `.dont/`, `.espectacular/`, `.pretender.toml`, `.testaruda/`
- Its own init command: `wai init`, `dont prime`, `ah init`, `pretender init`, `testaruda init`
- Its own installation path: `cargo install`, `brew install`, `scoop install`

Result: a new user runs **5+ install commands** and **5+ init commands** before seeing value. Configs are scattered across the repo root with no standard convention.

## The solution

**dulce-de-leche** (`ddl`) is a thin orchestrator that:

1. **Installs** all tools via the platform-native package manager (brew on macOS, cargo on Linux, scoop on Windows)
2. **Configures** them under a single `.ddl/` directory — no root pollution
3. **Migrates** existing configs from `.wai/`, `.dont/`, etc. into `.ddl/` (via symlinks in Phase 1)
4. **Reports** status across the whole toolset in one command
5. **Upgrades** everything to compatible versions in one step

It is **not** a package manager, a reimplementation of any tool, or a CLI launcher that replaces `wai`/`dont`/`ah`. Each tool keeps its own identity and CLI grammar. `ddl` is just the **orchestrator** — the "brew bundle" for charly-vibes.

## The `.ddl/` directory

```
.ddl/
  manifest.json          # tool versions, migration state, ddl version
  config.toml            # ddl's own config (not tool configs)
  wai/ -> ../.wai/              # symlink to .wai/
  dont/ -> ../.dont/            # symlink to .dont/
  ah/ -> ../.espectacular/      # symlink to .espectacular/
  pretender.toml -> ../.pretender.toml  # symlink
  testaruda/ -> ../.testaruda/  # symlink to .testaruda/
  fabbro/ -> ../.fabbro/        # symlink to .fabbro/
```

Data files (`.db`, stores) are git-ignored. Config files (`.toml`) are committed by default.

## Documentation

Full documentation is available at [charly-vibes.github.io/dulce-de-leche](https://charly-vibes.github.io/dulce-de-leche) (mdBook).

- [Introduction](https://charly-vibes.github.io/dulce-de-leche/introduction.html)
- [Installation](https://charly-vibes.github.io/dulce-de-leche/installation.html)
- [Quick Start](https://charly-vibes.github.io/dulce-de-leche/quick-start.html)
- [Commands](https://charly-vibes.github.io/dulce-de-leche/commands.html)
- [Architecture](https://charly-vibes.github.io/dulce-de-leche/architecture.html)
- [Development](https://charly-vibes.github.io/dulce-de-leche/development.html)

## Status

**Pre-release / design phase.** The concept has been reviewed using the Rule of 5 methodology. See [`docs/design.md`](docs/design.md) for the full design doc and [`docs/ecosystem-map.md`](docs/ecosystem-map.md) for the tool family overview.

## Commands

| Command | Description |
|---------|-------------|
| `ddl init` | Interactive or non-interactive bootstrap |
| `ddl install <tool>` | Install a single tool |
| `ddl status` | Cross-tool health overview |
| `ddl doctor` | Detailed diagnostics |
| `ddl version` | Show versions of ddl and all managed tools |
| `ddl upgrade` | Update all tools to latest compatible versions |
| `ddl migrate` | Move existing configs under `.ddl/` |
| `ddl scope` | Show which `.ddl/` is active |

## Project structure

```
.github/workflows/
  ci.yml          # CI: fmt, lint, test, build
  release.yml     # Release: cross-platform binaries, crates.io, brew, scoop
  docs.yml        # Docs: build and deploy mdBook to GitHub Pages

docs/
  book.toml       # mdBook configuration
  src/            # Documentation source files
  design.md       # Full design document (adversarial evaluation)
  ecosystem-map.md # Tool family overview

scripts/
  update-homebrew.py  # Homebrew formula updater (for CI)
  update-scoop.py     # Scoop manifest updater (for CI)

src/
  main.rs         # CLI entry point
  lib.rs          # Library root with module exports
  cli.rs          # Command structure (clap derive)
  error.rs        # Error types (miette + thiserror)
  manifest.rs     # Manifest management (.ddl/manifest.json)
  platform.rs     # Platform detection and tool registry

openspec/
  specs/          # Capability specifications
  changes/        # Change proposals and implementation plans
```

## Related repos

- [homebrew-charly](https://github.com/charly-vibes/homebrew-charly) — Homebrew tap
- [scoop-charly](https://github.com/charly-vibes/scoop-charly) — Scoop bucket
- [genesis-vibes](https://github.com/charly-vibes/genesis) — Shared infrastructure crate

## License

Apache 2.0 — see [LICENSE](LICENSE).