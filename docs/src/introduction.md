# dulce-de-leche (ddl)

**One command to install, configure, and update every charly-vibes tool.**

dulce-de-leche (`ddl`) is a thin orchestrator that bootstraps the entire
[charly-vibes](https://github.com/charly-vibes) tool ecosystem on any platform
— macOS, Linux, or Windows — with no prerequisites beyond the binary itself.

## The problem

The charly-vibes ecosystem has **6 active Rust CLI tools** (wai, dont, ah,
pretender, testaruda, fotos-mcp) plus fabbro (Go) and vampiro on the way.
Each tool has:

- Its own config file or directory
- Its own init command
- Its own installation path (cargo install, brew install, scoop install)

Result: a new user runs **5+ install commands** and **5+ init commands**
before seeing value. Configs are scattered across the repo root with no
standard convention.

## The solution

`ddl` provides a single binary that:

1. **Installs** all tools via the platform-native package manager
2. **Configures** them under a single `.ddl/` directory — no root pollution
3. **Migrates** existing configs into `.ddl/` (via symlinks in Phase 1)
4. **Reports** status across the whole toolset in one command
5. **Upgrades** everything to compatible versions in one step

It is **not** a package manager, a reimplementation of any tool, or a CLI
launcher that replaces `wai`/`dont`/`ah`. Each tool keeps its own identity
and CLI grammar. `ddl` is just the **orchestrator** — the "brew bundle" for
charly-vibes.

## Status

**Pre-release / design phase.** See [Implementation Status](./status.md) for
the current state of each component.

## License

Apache 2.0 — see [LICENSE](https://github.com/charly-vibes/dulce-de-leche/blob/main/LICENSE).