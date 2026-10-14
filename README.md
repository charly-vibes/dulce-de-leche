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

> **Why:** a dozen independent CLI tools mean a dozen install paths, config
> formats, and version drifts — `ddl` collapses that into one install, one
> config directory (`.ddl/`), and one health check across the ecosystem.
> **Status:** [stable](docs/src/status.md) · core commands (init/status/migrate/upgrade) shipped · [Ecosystem & motivation](docs/src/ecosystem.md) · [charly-vibes Tool Ecosystem](https://charly-vibes.github.io/dulce-de-leche/ecosystem-map.html)

```
   ddl init     →  installs & configures the whole toolset
   ddl status   →  shows health of all tools at a glance
   ddl migrate  →  moves existing configs under .ddl/
   ddl upgrade  →  updates everything to latest compatible versions
```

Repo root stays clean — everything lives under `.ddl/`.

Every repo in the fleet carries the `charly-vibes` + `dulce-de-leche` GitHub
topics — browse the whole set at
[topic:charly-vibes topic:dulce-de-leche](https://github.com/search?q=topic%3Acharly-vibes+topic%3Adulce-de-leche&type=repositories).

<!-- MANAGED-TOOLS:START -->
Managed tools (12):
- Core (2): wai, testaruda.
- Recommended (3): turu, dont, ah.
- Extension (7): pretender, vampiro, incitaciones, bd, openspec, specodelic, funkalization.
Run `ddl version` for the full picture. Additions and removals must update this block
(enforced by `tests/tool_registry_drift.rs`).
<!-- MANAGED-TOOLS:END -->

Canonical names and aliases — binary vs crate vs repo (agents: install by either
spelling, but the repo is always the third column):

| binary | crate | repository |
|---|---|---|
| `wai` | `wai-cli` | `charly-vibes/wai` |
| `dont` | `dont-cli` | `charly-vibes/dont` |
| `ah` | `espectacular` | `charly-vibes/espectacular` |
| `turu` | `whisper-vibes` | `charly-vibes/whisper` |
| `openspec` | `@fission-ai/openspec` (npm) | `fission-ai/openspec` |
| `ddl` (this tool) | `dulce-de-leche` | `charly-vibes/dulce-de-leche` |

`charly-vibes/ddl` does not exist — the repository is `charly-vibes/dulce-de-leche`.
This table lists only the tools whose binary and crate names mismatch and is
drift-tested in `tests/tool_registry_drift.rs`.

## Installation

```bash
# macOS (Homebrew)
brew tap charly-vibes/charly
brew install dulce-de-leche

# macOS & Linux (curl — resolves the latest release automatically)
V=$(basename "$(curl -fsSLI -o /dev/null -w '%{url_effective}' \
  https://github.com/charly-vibes/dulce-de-leche/releases/latest)" | sed 's/^v//')
TGT="$(uname -s | tr '[:upper:]' '[:lower:]')_$(uname -m | sed 's/^x86_64$/amd64/; s/^aarch64$/arm64/')"
curl -fsSL "https://github.com/charly-vibes/dulce-de-leche/releases/download/v${V}/ddl_${V}_${TGT}.tar.gz" | tar xz
chmod +x ddl && sudo mv ddl /usr/local/bin/

# Verify it runs
ddl --version

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

Eleven independent CLI tools — each with its own config directory, init
command, and install path — mean a new user runs 5+ install and 5+ init
commands before seeing value, with configs scattered across the repo root.
Full story: [Introduction](https://charly-vibes.github.io/dulce-de-leche/introduction.html).

## The solution

**dulce-de-leche** (`ddl`) is a thin orchestrator — the "brew bundle" for
charly-vibes — that **installs** every tool (prebuilt binaries first, cargo
fallback, npm for incitaciones), **configures** them under a single `.ddl/`
directory, **migrates** legacy configs into it, **reports** status in one
command, and **upgrades** everything in one step.

It is **not** a package manager or a reimplementation of any tool: each tool
keeps its own identity and CLI grammar; `ddl` just orchestrates them.

## The `.ddl/` directory

```
.ddl/
  manifest.json          # tool versions, migration state, ddl version
  config.toml            # ddl's own config (not tool configs)
  wai/                   # wai config files live here
  dont/                  # dont config files live here
  ah/                    # ah config files live here
  pretender.toml         # pretender config file
  testaruda/             # testaruda config files
  vampiro/               # vampiro config files
  specodelic/            # specodelic config files
```

Four tools are **install-only** — their state lives outside `.ddl/`, so there
is nothing to migrate: `bd` (manages repo-local `.beads/` itself), `openspec`
(manages `openspec/` itself), `incitaciones` (skills land in
`~/.agents/skills/`), and `turu` (state in `~/.whisper/`).

Legacy directories (`.wai/`, `.dont/`, etc.) become symlinks pointing to `.ddl/`:

```
.wai/            -> .ddl/wai/            # symlink to .ddl/
.dont/           -> .ddl/dont/           # symlink to .ddl/
.espectacular/   -> .ddl/ah/             # symlink to .ddl/
.pretender.toml  -> .ddl/pretender.toml  # symlink to .ddl/
.testaruda/      -> .ddl/testaruda/     # symlink to .ddl/
.vampiro/        -> .ddl/vampiro/        # symlink to .ddl/
.specs/          -> .ddl/specodelic/     # symlink to .ddl/
```

## Documentation

Full documentation is available at [charly-vibes.github.io/dulce-de-leche](https://charly-vibes.github.io/dulce-de-leche) (mdBook).

- [Introduction](https://charly-vibes.github.io/dulce-de-leche/introduction.html)
- [Installation](https://charly-vibes.github.io/dulce-de-leche/installation.html)
- [Quick Start](https://charly-vibes.github.io/dulce-de-leche/quick-start.html)
- [Commands](https://charly-vibes.github.io/dulce-de-leche/commands.html)
- [Architecture](https://charly-vibes.github.io/dulce-de-leche/architecture.html)
- [Development](https://charly-vibes.github.io/dulce-de-leche/development.html)

## Documentation provenance

Rendered docs are generated artifacts; the repository sources are
authoritative — edit the repo sources, not the rendered output.

## Status

**v0.8.0** — orchestrator shipped and published (crates.io, GitHub
releases); installs are binary-first on every platform with `cargo install`
fallback, and the registry holds 12 managed tools (bd and openspec joined).
Brew/scoop no longer influence ddl's install
decisions ([DDL-ei3]). Upstream release coverage is tracked in
[`docs/ecosystem-map.md`](docs/ecosystem-map.md). Notes:

- Real tap formulas exist for manual installs: wai, testaruda, vampiro,
  turu — dont is still a placeholder.
- Tools without published release binaries fall back to `cargo install`
  automatically.
- **`bd` has no cargo fallback** (the `beads` crate on crates.io is an
  unrelated package) — it installs strictly from gastownhall/beads release
  binaries.

## Commands

| Command | Description |
|---------|-------------|
| `ddl init` | Interactive or non-interactive bootstrap |
| `ddl install <tool>` | Install a single tool |
| `ddl catalog` | Show the tool catalog — what exists, what it does, how to invoke |
| `ddl feedback <kind>` | File a feedback issue (bug|feature|question|chore) via `gh` |
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

[DDL-ei3]: https://github.com/charly-vibes/dulce-de-leche/issues/DDL-ei3