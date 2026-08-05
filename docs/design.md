# dulce-de-leche — Design Document

> One `.ddl/` directory to configure all charly-vibes tools.
> Status: **Implemented**

## Table of Contents

- [1. Principles](#1-principles)
- [2. Scope](#2-scope)
- [3. Directory Layout](#3-directory-layout)
- [4. Commands](#4-commands)
- [5. Migration Model](#5-migration-model)
- [6. Version Management](#6-version-management)
- [7. Genesis Integration](#7-genesis-integration)
- [8. Installation Strategy](#8-installation-strategy)
- [9. Rule of 5 Review Findings](#9-rule-of-5-review-findings)
- [10. Open Questions](#10-open-questions)

## 1. Principles

1. **Orchestrate, don't replace.** ddl orchestrates existing tools. It does not reimplement `wai status`, `dont conclude`, or `ah check`. Each tool keeps its own CLI grammar and identity.
2. **Genesis-first.** ddl depends on `genesis-vibes` and uses its `ConfigFile`, `StatusContributor`, and `DoctorRunner` traits. No config discovery is reimplemented.
3. **Progressive adoption.** Phase 1 uses symlinks — no tool patches needed. Phase 2 adds deep integration via genesis.
4. **Idempotent and reversible.** Every command can be re-run safely. `ddl migrate --undo` restores the previous layout.
5. **Platform-native installation.** ddl calls `brew`, `cargo`, or `scoop` — it does not reimplement package resolution.

## 2. Scope

### In scope

| Concern | How |
|---------|-----|
| Single `.ddl/` config directory | Symlink farm (Phase 1), config proxy (Phase 2) |
| One-command install of all tools | `ddl install` → detects platform → `brew`/`cargo`/`scoop` |
| One-command init of all tools | `ddl init` → runs each tool's init/prime |
| Cross-tool status dashboard | `ddl status` → aggregates `StatusContributor` from each tool |
| Cross-tool doctor | `ddl doctor` → aggregates `DoctorRunner` from each tool |
| Config migration | `ddl migrate` → moves configs under `.ddl/` |
| Global upgrade | `ddl upgrade` → updates all tools to compatible versions |
| Version manifest | `.ddl/manifest.json` tracks installed versions |

### Out of scope (explicitly)

| Concern | Why not |
|---------|---------|
| Package resolver | Existing tools (brew, cargo, scoop) do this better |
| Unified CLI launcher | Each tool has its own CLI grammar — would break identity and docs |
| Database merge | dont and testaruda use independent stores (Cozo, SQLite) — cannot merge |
| Configuration language | TOML is already the ecosystem convention (genesis) |
| CI/CD integration | Each tool handles its own CI (pretender generates GitHub Actions, etc.) |

## 3. Directory Layout

### Phase 1: Symlink farm

```
.ddl/
  manifest.json          # {"ddl_version": "1.0.0", "tool_versions": {...}, "migration_state": "phase1"}
  config.toml            # ddl's own config: install_source, auto_upgrade, gitignore_strategy

  # Symlinks to existing configs
  wai/ -> ../.wai/              # symlink to .wai/
  dont/ -> ../.dont/            # symlink to .dont/
  ah/ -> ../.espectacular/      # symlink to .espectacular/
  pretender.toml -> ../.pretender.toml  # symlink
  testaruda/ -> ../.testaruda/  # symlink to .testaruda/

  # ddl-specific metadata
  install-log.json      # record of what was installed and when
  doctor-cache.json     # cached doctor results for fast status
```

### Phase 2: Native config proxy

In Phase 2, each tool (via genesis) learns to check `.ddl/<tool>/` before its legacy location:

```
.ddl/
  manifest.json
  config.toml

  # Actual config files, not symlinks
  wai/
    config.toml
  dont/
    config.toml
    claims.db              # Cozo DB, gitignored
  ah/
    config.toml
    spec-trace.json
  pretender.toml           # pretender only has one file
  testaruda/
    config.toml
    store.db               # SQLite, gitignored
  fabbro/
    config.toml
```

### `.gitignore` template

```gitignore
# dulce-de-leche — data files (do not commit)
.ddl/**/*.db
.ddl/**/store/
.ddl/install-log.json
.ddl/doctor-cache.json
```

`ddl init` asks: "Add `.gitignore` entries? [Y/n]" and also offers to commit config files.

## 4. Commands

### `ddl init` — First-time setup

```bash
ddl init
# 1. Detects platform (macOS/Linux/Windows)
# 2. Asks which tools to install (default: all active)
# 3. Runs: brew/cargo/scoop install for each selected tool
# 4. Creates .ddl/ directory structure
# 5. Runs: wai init, dont prime, ah init, pretender init, testaruda init
# 6. Writes .ddl/manifest.json
# 7. Optionally adds .gitignore entries
```

Flags: `--no-install` (only configure), `--tools wai,dont` (select subset), `--yes` (non-interactive)

### `ddl install <tool>` — Install a single tool

```bash
ddl install wai
# Detects platform → calls platform installer → records in manifest
```

Platform strategy:

| Platform | Tool | Command |
|----------|------|---------|
| macOS | wai | `brew install wai` |
| macOS | dont | `brew install dont` |
| macOS | ah | `brew install ah` |
| macOS | pretender | `brew install pretender` |
| macOS | testaruda | `brew install testaruda` |
| macOS | fotos-mcp | `brew install fotos-mcp` |
| macOS | fabbro | `brew install fabbro` |
| Linux | any | `cargo install <crate>` |
| Windows | any | `scoop install <name>` |

`ddl install` checks prerequisites first (`rustc`, `brew`, `scoop`) and suggests installation if missing.

### `ddl status` — Cross-tool dashboard

```bash
ddl status
# Shows for each tool:
#   ✓ installed  ✓ configured  ✓ healthy
#   ✗ not installed  (suggests: ddl install wai)
#   ⚠ version outdated (suggests: ddl upgrade)
#
# Aggregates genesis StatusContributor for each registered tool.
```

### `ddl doctor` — Cross-tool diagnostics

```bash
ddl doctor
# Runs doctor checks for all tools, shows aggregate report.
# Uses genesis DoctorRunner for the fix-verify cycle.
```

### `ddl upgrade` — Update everything

```bash
ddl upgrade
# 1. Reads .ddl/manifest.json for current versions
# 2. For each tool: calls platform updater (brew upgrade / cargo install / scoop update)
# 3. Verifies new versions
# 4. Updates manifest.json
```

### `ddl migrate` — Move configs under `.ddl/`

Phase 1 behavior:

```bash
ddl migrate
# For each installed tool:
#   1. Detects existing config location
#   2. Moves it to .ddl/<tool>/
#   3. Creates a symlink at the old location for backwards compat
#   4. Updates manifest.json migration_state
#
# Idempotent: re-running has no effect
# Transactional: failures roll back created symlinks
```

Flags: `--undo` (restores previous layout, removes symlinks)

### `ddl scope` — Show active `.ddl/`

```bash
ddl scope
# Shows which .ddl/ is active (walks up from CWD like git)
# /home/user/projects/foo/.ddl/  (nearest ancestor wins)
```

### `ddl version` — Show versions

```bash
ddl version
# ddl: 0.1.0
# Managed tools:
#   wai:        2026.5.1  up to date
#   dont:       0.2.2     up to date
#   ah:         0.3.0     update available (0.3.1)
#   pretender:  0.3.1     up to date
#   testaruda:  0.2.6     up to date
```

## 5. Migration Model

Two-phase approach:

### Phase 1: Symlink farm (recommended for initial release)

```
Before:                    After:
.wai/config.toml           .ddl/wai/ -> ../.wai/   (symlink)
.dont/claims.db            .ddl/dont/ -> ../.dont/  (symlink)
.espectacular/             .ddl/ah/ -> ../.espectacular/ (symlink)
```

- **No tool patches needed.** Each tool continues reading from its original location.
- **`ddl status`** reads from the original locations too (via symlink).
- **Risk:** Zero. Symlinks are transparent to file reads.
- **Downside:** Root directory is not truly clean — original dirs still exist at root. But a `ls -la` shows them as broken symlinks if the user deletes the originals, so it's a gentler transition.

Phase 1 migration steps:
1. Create `.ddl/<tool>/` directory
2. Move `.wai/` contents → `.ddl/wai/`
3. Create symlink `.wai/` → `.ddl/wai/`
4. Repeat for each tool

### Phase 2: Deep integration (requires genesis patches)

- Each tool's `ConfigFile::discover()` implementation checks `.ddl/<tool>/` first, then falls back to legacy location.
- `ddl migrate --deep` copies/removes files from legacy locations (no symlinks needed).
- Requires coordinated releases of each tool with updated genesis.

**Decision:** Start with Phase 1. Re-evaluate after 3 months of real usage.

## 6. Version Management

`.ddl/manifest.json`:

```json
{
  "ddl_version": "1.0.0",
  "migration_state": "phase1",
  "tools": {
    "wai": {
      "installed": "2026.5.1",
      "source": "brew",
      "compatible": ">=2026.3.0"
    },
    "dont": {
      "installed": "0.2.2",
      "source": "brew",
      "compatible": ">=0.2.0"
    },
    "ah": {
      "installed": "0.3.0",
      "source": "brew",
      "compatible": ">=0.2.0"
    }
  }
}
```

Each ddl release ships with a `compatible_versions.json` embedded in the binary:

```json
{
  "wai": ">=2026.3.0",
  "dont": ">=0.2.0",
  "ah": ">=0.2.0",
  "pretender": ">=0.3.0",
  "testaruda": ">=0.2.0",
  "fotos-mcp": ">=0.3.0"
}
```

`ddl install` refuses to install a version outside the compatible range. `ddl upgrade` skips tools at their max compatible version.

## 7. Genesis Integration

ddl depends on `genesis-vibes = "0.3"` and uses:

| Genesis module | How ddl uses it |
|---|---|
| `ConfigFile` / `ConfigRegistry` | Registers each tool's config location and validator |
| `StatusContributor` + `StatusBuilder` | Aggregates health state across all tools in `ddl status` |
| `DoctorRunner` + `DoctorReport` | Runs doctor checks for all tools in `ddl doctor` |
| `Scaffold` | Creates `.ddl/` directory and config files in `ddl init` |
| `suggestions` | `DidYouMean` for misspelled tool names in `ddl install` |
| `envelope` | All ddl commands return structured JSON output |
| `suite_linter` | Lints cross-tool config consistency |
| `fixture` | Test helpers for integration tests |

Data flow for `ddl status`:

```
ddl status
  → reads config from .ddl/config.toml (via genesis ConfigFile)
  → instantiates StatusBuilder
  → for each registered tool:
      → loads tool config from .ddl/<tool>/ (via genesis ConfigStore)
      → runs tool's check (subprocess or genesis StatusContributor trait)
      → collects result
  → renders aggregate StatusReport via genesis envelope
```

## 8. Installation Strategy

### Installing ddl itself

```bash
# Homebrew (macOS/Linux)
brew tap charly-vibes/charly
brew install dulce-de-leche

# Cargo (any platform)
cargo install dulce-de-leche

# Scoop (Windows)
scoop bucket add charly https://github.com/charly-vibes/scoop-charly.git
scoop install dulce-de-leche

# Binary release
curl -fsSL https://github.com/charly-vibes/dulce-de-leche/releases/latest/download/ddl-$(uname -s)-$(uname -m).tar.gz | tar xz
```

### First-run flow

```
$ ddl init
╭──────────────────────────────────────╮
│  dulce-de-leche — charly-vibes       │
│  bundle orchestrator v1.0.0          │
╰──────────────────────────────────────╯

Detected platform: macOS (arm64)
Available package manager: brew

Which tools would you like to install?
  ✓ wai (workflow manager)
  ✓ dont (epistemic discipline)
  ✓ ah (spec-test verification)
  ✓ pretender (code quality)
  ✓ testaruda (test selection)
  ✓ fotos-mcp (screenshot MCP)
  ✓ fabbro (code review)
  [Select all / none / custom]

  → all

Installing 7 tools...
  ✓ brew install wai
  ✓ brew install dont
  ✓ brew install ah
  ✓ brew install pretender
  ✓ brew install testaruda
  ✓ brew install fotos-mcp
  ✓ brew install fabbro

Configuring...
  ✓ .ddl/ created
  ✓ wai init
  ✓ dont prime
  ✓ ah init
  ✓ pretender init
  ✓ testaruda init

Git integration:
  Add .gitignore entries for .ddl data files? [Y/n]
  → Y
  ✓ .gitignore updated

Done! Run `ddl status` to verify everything.
```

## 9. Rule of 5 Review Findings

The ddl concept was reviewed using Steve Yegge's Rule of 5 methodology (via the `rule-of-5-universal` skill). Summary of findings:

### Critical (must fix before proceeding)

- **Genesis compatibility.** ddl must depend on `genesis-vibes` and use its `ConfigFile`/`StatusContributor` traits. Do NOT reimplement config discovery.
- **Workspace nesting ambiguity.** `.ddl/` discovery must use git-like ancestor walk (nearest ancestor wins). `ddl scope` must show the active `.ddl/`.

### High (should fix before proceeding)

- **Scope boundary: orchestrator vs package manager.** ddl invokes brew/cargo/scoop — it does NOT reimplement package resolution. This is a hard boundary.
- **Migration model must be decided first.** Phase 1 = symlink farm. Phase 2 = native config proxy. Implement Phase 1 first.
- **Version coupling.** ddl ships with a compatible-version matrix. It refuses to install incompatible versions.
- **Offline/sans-toolchain scenario.** `ddl doctor` checks prerequisites before `ddl install`. Binary downloads as fallback.

### Medium (address before v1.0)

- Database migration isolation (each tool keeps its own store under `.ddl/<tool>/`)
- Partial migration handling (idempotent, transactional, clear output)
- Tool version skew (ddl tracks version of each tool, refuses to migrate incompatible versions)
- Cross-platform install strategy (decision matrix per platform)
- Naming: `ddl` collides with SQL DDL — use full `dulce-de-leche` for crate name, `ddl` for binary

### Low (nice to have)

- Telemetry-free self-update: `ddl upgrade`
- Shell compatibility: `.ddl/` structure should be simple enough for shell scripts to read

## 10. Open Questions

1. **Does `ddl migrate` also migrate gitignores?** If a project already has `.wai/` in `.gitignore`, after migration that entry becomes stale. Should `ddl migrate` update `.gitignore` automatically?

2. **Is `ddl scope` needed at all?** Could simply use the working directory convention (run `ddl` where `.ddl/` lives). Most charly-vibes projects are single-tool, not monorepos.

3. **Should `ddl status` call tool subprocesses or link against genesis traits?** Subprocesses are safer (no version skew between shared libraries) but slower. Genesis link is faster. Phase 1: subprocesses. Phase 2: genesis link with version check.

4. **What about the `.wai/`, `.dont/` etc. directories that already exist in the charly-vibes monorepo itself?** ddl should handle its own dogfooding: `ddl init` in the charly-vibes root should migrate all existing configs under `.ddl/`.

5. **Should ddl support "profiles"?** E.g., `ddl install --profile minimal` (wai only) vs `ddl install --profile full` (everything). This lowers the "minimum viable install" bar.

6. **What about non-Rust tools?** fabbro (Go), incitaciones (markdown), atril (web). They don't use genesis. Should ddl manage them at all, or only the Rust tools? Proposal: manage them in Phase 1 as "external" — install via brew, config via `.ddl/<tool>/`, status via subprocess parsing.
