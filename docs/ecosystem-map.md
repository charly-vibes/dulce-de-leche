# charly-vibes Tool Ecosystem

> What ddl manages, and how each tool fits in.

## Inclusion criteria

ddl manages a tool when all of the following hold:

1. It is an installable CLI — users run it in a terminal (not a desktop app, library, static site, or content repo).
2. It has (or is about to get) a distribution channel in the charly-vibes tap/bucket.
3. It has a config convention ddl can manage (or is explicitly install-only, like fotos-mcp).

Explicitly excluded sibling repos, and why:

| Repo | Why excluded |
|------|--------------|
| genesis | Library crate — shared infrastructure, not user-facing |
| fotos (cask) | Desktop Tauri app; fotos-mcp is the CLI-managed part |
| incitaciones | Prompt/skill collection — no installable binary |
| bichos | Bio-mimetic QA framework — no CLI yet |
| atril | Static web viewer |
| paranoid | Android app |
| canticos, paseos, khipu, rizomas, livin, jams, miblioteca, microdancing, nayra, ruta, superficies, tRAGar, crua, charly-vibes, charly-vibes.github.io | Content, personal scripts, org, or infra repos — no installable CLI |

## Active Tools

### Rust CLI tools

| Tool | Binary | Crates.io | Genesis consumer? | Purpose |
|------|--------|-----------|-------------------|---------|
| **wai** | `wai` | `wai-cli` | ✓ | Workflow manager for AI development. PARA-based artifact organization, phase tracking, handoffs. |
| **dont** | `dont`, `dt` | `dont-cli` | ✓ | Epistemic discipline state machine. Claims, evidence, vocabulary tracking. Cozo DB backend. |
| **espectacular** | `ah` | `espectacular` | ✓ | Behavioral verification. Spec-test correspondence enforcement. |
| **pretender** | `pretender` | `pretender` | ✓ | Multi-language structural code quality. Tree-sitter-based complexity, duplication, mutation testing. |
| **testaruda** | `testaruda`, `testaruda-adapter-rust`, `testaruda-adapter-python` | `testaruda` | ✓ | Test selection engine. Ascent Datalog + provenance semiring. SQLite store. |
| **vampiro** | `vampiro` | `vampiro` | ✓ | Cross-language composition checking at call, module, effect, law, retry, resource, and trust boundaries. Crate workspace with per-language tracers. |
| **fotos-mcp** | `fotos-mcp` | `fotos-mcp` | ✗ | MCP server for Fotos screenshot tool. IPC bridge. Separate from main Tauri app. Install-only under ddl (see config tables below). |

### Non-Rust tools

| Tool | Language | In homebrew? | Notes |
|------|----------|-------------|-------|
| **fabbro** | Go | ✓ (`fabbro`) | Local-first code review annotation with TUI. |
| **fotos** | Rust/Tauri | ✓ (cask `fotos`) | Desktop screenshot app with AI analysis. Not a CLI tool. |

## Config file locations

Before ddl:

| Tool | Config location | Format | Notes |
|------|----------------|--------|-------|
| wai | `.wai/` | TOML + directory | PARA structure |
| dont | `.dont/` | Cozo DB + TOML | Heavyweight store |
| espectacular | `.espectacular/` | JSON + TOML | Spec traces |
| pretender | `.pretender.toml` | TOML | Single file |
| testaruda | `.testaruda/` | SQLite + TOML | Dependency graph store |
| vampiro | `.vampiro/` | TOML | Composition config (`config.toml`) |
| fabbro | `.fabbro/` | Directory | Session storage (`sessions/`) |

After ddl migration (Phase 1):

| Tool | Config location | Backwards compat |
|------|----------------|-----------------|
| wai | `.ddl/wai/` → `.wai/` | ✓ symlink |
| dont | `.ddl/dont/` → `.dont/` | ✓ symlink |
| espectacular | `.ddl/ah/` → `.espectacular/` | ✓ symlink |
| pretender | `.ddl/pretender.toml` → `.pretender.toml` | ✓ symlink |
| testaruda | `.ddl/testaruda/` → `.testaruda/` | ✓ symlink |
| vampiro | `.ddl/vampiro/` → `.vampiro/` | ✓ symlink |
| fabbro | `.ddl/fabbro/` → `.fabbro/` | ✓ symlink |

`fotos-mcp` is install-only — its MCP client config lives outside `.ddl/`, so there is nothing to migrate.

## Shared infrastructure (genesis-vibes)

All Rust tools (except fotos-mcp) depend on `genesis-vibes = "0.6"`. Genesis provides:

| Module | Used by | Purpose |
|--------|---------|---------|
| `envelope` | All | Structured JSON CLI output |
| `suggestions` | wai, dont | Typo correction (`DidYouMean`) |
| `managed_block` | wai, dont, ah, testaruda, vampiro | `<!-- BLOCK:START -->` injection |
| `config` | All | `ConfigFile` trait, `ConfigRegistry` |
| `guide` | All | CLI dispatch, error handling, verbosity |
| `status` | All | `StatusContributor` trait, health aggregation |
| `doctor` | All | `DoctorRunner` with auto-fix |
| `scaffold` | All | `init` command standardization |
| `suite_linter` | testaruda | Cross-tool config linting |
| `fixture` | All | Test helpers |

## Distribution channels

| Channel | Tools | Maintainer |
|---------|-------|------------|
| crates.io | All Rust tools | Individual repos |
| Homebrew | wai, ah, dont, pretender, fotos-mcp, fabbro, dulce-de-leche, fotos (cask) — testaruda & vampiro formulas not yet scaffolded | `homebrew-charly` tap |
| Scoop | wai, ah, dont, pretender, fotos-mcp, fotos, fabbro, dulce-de-leche | `scoop-charly` bucket |
| GitHub Releases | All Rust tools | Individual repos |

## Homebrew formulas (current state)

From `homebrew-charly/` (verified 2026-09-01):

| Formula | Version | Has real SHA? | Notes |
|---------|---------|---------------|-------|
| `wai.rb` | 2026.5.3 | ✓ | Real release (upstream tag v2026.8.5 exists — formula update pending) |
| `fotos-mcp.rb` | 0.3.0 | ✓ | Real release |
| `dulce-de-leche.rb` | 0.1.0 | ✗ placeholder | Stale — ddl is at v0.3.0 |
| `ah.rb` | 0.0.0 | ✗ placeholder | Not published yet (upstream v0.5.0 tagged) |
| `dont.rb` | 0.0.0 | ✗ placeholder | Not published yet (upstream v0.3.0 tagged) |
| `pretender.rb` | 0.0.0 | ✗ placeholder | Not published yet (upstream v0.5.0 tagged) |
| `fabbro.rb` | 0.0.0 | ✗ placeholder | Not published yet |
| `testaruda.rb` | — | — | Formula not scaffolded yet (upstream v0.4.0 tagged) |
| `vampiro.rb` | — | — | Formula not scaffolded yet (upstream v0.4.0 tagged) |
| `fotos.rb` (cask) | — | ✓ | Real release |

Only **wai** and **fotos-mcp** have real releases published. ddl automatically falls back to `cargo install` for tools whose formulas are placeholders (`is_placeholder_formula` in `src/installer.rs`), so `ddl init` still works — brew users just get the cargo path for those tools.
