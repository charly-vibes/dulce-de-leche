---
tags: [ecosystem-map, reference]
---

# charly-vibes Tool Ecosystem

> What ddl manages, and how each tool fits in.

## Active Tools

### Rust CLI tools

| Tool | Binary | Crates.io | Genesis consumer? | Purpose |
|------|--------|-----------|-------------------|---------|
| **wai** | `wai` | `wai-cli` | ✓ | Workflow manager for AI development. PARA-based artifact organization, phase tracking, handoffs. |
| **dont** | `dont`, `dt` | `dont-cli` | ✓ | Epistemic discipline state machine. Claims, evidence, vocabulary tracking. Cozo DB backend. |
| **espectacular** | `ah` | `espectacular` | ✓ | Behavioral verification. Spec-test correspondence enforcement. |
| **pretender** | `pretender` | `pretender` | ✓ | Multi-language structural code quality. Tree-sitter-based complexity, duplication, mutation testing. |
| **testaruda** | `testaruda`, `testaruda-adapter-rust`, `testaruda-adapter-python` | `testaruda` | ✓ | Test selection engine. Ascent Datalog + provenance semiring. SQLite store. |
| **fotos-mcp** | `fotos-mcp` | `fotos-mcp` | ✗ | MCP server for Fotos screenshot tool. IPC bridge. Separate from main Tauri app. |

### Non-Rust tools

| Tool | Language | In homebrew? | Notes |
|------|----------|-------------|-------|
| **fabbro** | Go | ✓ (`fabbro`) | Local-first code review annotation with TUI. |
| **fotos** | Rust/Tauri | ✓ (cask `fotos`) | Desktop screenshot app with AI analysis. Not a CLI tool. |
| **bichos** | ? | ✗ | Bio-mimetic QA framework. Not yet in homebrew. |
| **atril** | ? | ✗ | Web viewer for specs/issues. Static site. |
| **incitaciones** | markdown | ✗ | Prompt collection. Distributed as skills. |
| **canticos** | shell | ✗ | Personal scripts. Not distributed. |

### Pre-implementation

| Tool | Status | Notes |
|------|--------|-------|
| **vampiro** | Spec stage (EARS v1.3.0 approved) | Cross-boundary composition checking. Will use genesis. |

## Config file locations

Before ddl:

| Tool | Config location | Format | Notes |
|------|----------------|--------|-------|
| wai | `.wai/` | TOML + directory | PARA structure |
| dont | `.dont/` | Cozo DB + TOML | Heavyweight store |
| espectacular | `.espectacular/` | JSON + TOML | Spec traces |
| pretender | `.pretender.toml` | TOML | Single file |
| testaruda | `.testaruda/` | SQLite + TOML | Dependency graph store |
| fabbro | `fabbro.toml` | TOML | Go tool |

After ddl migration (Phase 1):

| Tool | Config location | Backwards compat |
|------|----------------|-----------------|
| wai | `.ddl/wai/` → `.wai/` | ✓ symlink |
| dont | `.ddl/dont/` → `.dont/` | ✓ symlink |
| espectacular | `.ddl/ah/` → `.espectacular/` | ✓ symlink |
| pretender | `.ddl/pretender.toml` → `.pretender.toml` | ✓ symlink |
| testaruda | `.ddl/testaruda/` → `.testaruda/` | ✓ symlink |
| fabbro | `.ddl/fabbro/` → `fabbro.toml` | ✓ symlink |

## Shared infrastructure (genesis-vibes)

All Rust tools (except fotos-mcp) depend on `genesis-vibes = "0.3"`. Genesis provides:

| Module | Used by | Purpose |
|--------|---------|---------|
| `envelope` | All | Structured JSON CLI output |
| `suggestions` | wai, dont | Typo correction (`DidYouMean`) |
| `managed_block` | wai, dont, ah, testaruda | `<!-- BLOCK:START -->` injection |
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
| Homebrew | wai, ah, dont, pretender, testaruda, fotos-mcp, fabbro, fotos (cask) | `homebrew-charly` tap |
| Scoop | wai, ah, dont, pretender, fotos-mcp, fotos, fabbro | `scoop-charly` bucket |
| GitHub Releases | All Rust tools | Individual repos |

## Homebrew formulas (current state)

From `homebrew-charly/`:

| Formula | Version | Has real SHA? | Notes |
|---------|---------|---------------|-------|
| `wai.rb` | 2026.5.1 | ✓ | Real release |
| `ah.rb` | 0.0.0 | ✗ placeholder | Not published yet |
| `dont.rb` | 0.0.0 | ✗ placeholder | Not published yet |
| `pretender.rb` | 0.0.0 | ✗ placeholder | Not published yet |
| `fabbro.rb` | 0.0.0 | ✗ placeholder | Not published yet |
| `fotos-mcp.rb` | 0.3.0 | ✓ | Real release |
| `fotos.rb` (cask) | ? | ✓ | Real release |

Only **wai** and **fotos-mcp** have real releases published. The rest are scaffolded but not shipped.
