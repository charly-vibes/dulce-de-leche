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
| **whisper** | `turu` (aliases: `turututu`, `whisper`) | `whisper-vibes` | ✓ | Deterministic knowledge workspace management. Repo-local `.whisper/` + global `~/.whisper/` routing. Install-only under ddl — its state is home-dir global, not repo-local legacy config. |

### Non-Rust tools

| Tool | Language | In homebrew? | Notes |
|------|----------|-------------|-------|
| **fabbro** | Go | ✓ (`fabbro`) | Local-first code review annotation with TUI. |
| **fotos** | Rust/Tauri | ✓ (cask `fotos`) | Desktop screenshot app with AI analysis. Not a CLI tool. |
| **incitaciones** | TypeScript/npm | npm (`incitaciones`) | Prompt/skill collection for CLI LLM tools. Install-only — skills land in `~/.agents/skills/` or `.agents/skills/`; `ddl init` checks for global skills and prompts if missing. |
| **whisper** | Rust | ✓ (`turu`) | Knowledge-workspace CLI (`turu` on crates.io as `whisper-vibes`). Install-only — state lives in `~/.whisper/`, not a repo-local legacy config. |

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
| Homebrew | wai, ah, dont, pretender, testaruda, vampiro, fotos-mcp, fabbro, turu, dulce-de-leche, fotos (cask) — dont & fabbro formulas still placeholder stubs | `homebrew-charly` tap |
| Scoop | wai, ah, dont, pretender, testaruda, vampiro, fotos-mcp, fotos, fabbro, turu, dulce-de-leche | `scoop-charly` bucket |
| GitHub Releases | All Rust tools | Individual repos |

## Homebrew formulas (current state)

From `homebrew-charly/`:

| Formula | Version | Has real SHA? | Notes |
|---------|---------|---------------|-------|
| `wai.rb` | 2026.8.5 | ✓ | Real release |
| `fotos-mcp.rb` | 0.3.0 | ✓ | Real release |
| `testaruda.rb` | 0.4.0 | ✓ | Real release (engine + rust/python adapters) |
| `vampiro.rb` | 0.4.0 | ✓ | Real release |
| `turu.rb` | 0.3.0 | ✓ | Real release (auto-updated by whisper's release workflow) |
| `ah.rb` | 0.5.0 | ✓ | Real release (assets under `espectacular` releases) |
| `pretender.rb` | 0.5.0 | ✓ | Real release |
| `dulce-de-leche.rb` | 0.3.0 | ✓ | Real release |
| `dont.rb` | 0.0.0 | ✗ placeholder | Blocked: no GitHub release for dont yet (tag v0.3.0 exists locally) |
| `fabbro.rb` | 0.0.0 | ✗ placeholder | Blocked: fabbro has no tagged releases |
| `fotos.rb` (cask) | — | ✓ | Real release |

Scoop mirrors the same versions for Windows. All formula hashes were computed from downloaded release assets and cross-verified against each release's published `checksums.txt`.

Only **dont** and **fabbro** remain blocked on upstream releases. ddl automatically falls back to `cargo install` for tools whose release binaries are not yet published, so `ddl init` works either way. (Since DDL-ei3, brew/scoop are not part of ddl's install decisions — the tap and bucket below serve manual installs.)
