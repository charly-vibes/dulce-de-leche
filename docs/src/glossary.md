# Glossary

Terms used throughout this book that are not part of ddl's own CLI surface.

| Term | Meaning |
|------|---------|
| **Managed tool** | A tool ddl can install, configure, and upgrade — listed by `ddl catalog` and in [Ecosystem Development](./ecosystem.md). |
| **Install-only tool** | A managed tool whose state lives outside `.ddl/` (nothing to migrate): `bd`, `openspec`, `incitaciones`, `turu`. |
| **`.ddl/` directory** | The single backing store for tool configs and ddl's manifest (`manifest.json`, `config.toml`) at a repository root. |
| **Symlink farm** | Phase-1 migration layout: a tool's legacy config path (`.wai/`, `.dont/`, …) becomes a symlink pointing into `.ddl/<tool>/`, which holds the real files. |
| **Root pollution** | Scattering per-tool config directories across a repository root instead of consolidating them under `.ddl/`. |
| **Init cascade** | The ordered run of each tool's own init command during `ddl init` (e.g. openspec before `ah`), with resume-until-green semantics. |
| **Gates** | `ddl init --gates` wiring: lefthook pre-commit/pre-push checks, `.gitignore` entries for tool data dirs, and beads no-db stamping. |
| **Hard gates** | Lefthook hooks that fail the commit/push (not merely warn) when a check fails. |
| **Smoke gate** | CI end-to-end verification that release binaries install and run on all three platforms (macOS, Linux, Windows). |
| **Dogfooding** | ddl (and the charly-vibes fleet) running its own conventions and tools on itself; enforced by conformance tests. |
| **Genesis** | Shared infrastructure crate (`genesis-vibes/genesis`) used by ecosystem tools for CLI scaffolding, completions, and version management. |
| **Binary name / crate name** | The executable you run vs. the package you install (`ah` vs. `espectacular`, `turu` vs. `whisper-vibes`). See the alias table in [Commands](./commands.md). |