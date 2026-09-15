# Simplify Install Chain: Binary-First, Cargo Fallback

## Why

ddl's documented design (project.md: "binary download → cargo install → brew/scoop install") and its actual behavior have drifted apart. `best_install_method()` currently prefers Homebrew on macOS (when the formula is real), Scoop on Windows, and Cargo on Linux — with binary download as the last resort everywhere. This creates a three-way decision matrix with a placeholder-formula special case (`is_placeholder_formula`), and produces different install channels for the same tool on different machines — the root of channel-conflict risk (shadowed binaries, upgrades routed through a different channel than the one that installed).

User decision (2026-09-15, ticket DDL-ei3): **prebuilt binary first on every platform; cargo as the fallback when a release binary is unavailable and cargo is installed; npm only for incitaciones.** Homebrew and Scoop leave ddl's install decision path entirely. Tap formulas and scoop manifests remain published as ecosystem infrastructure for users who prefer those channels manually — they simply no longer influence ddl's behavior.

## What Changes

- `best_install_method()` returns Binary for every platform; Cargo becomes a **runtime** fallback when the binary download fails with a release-not-found error (404) and cargo is available
- The old spec rule "on 404, report an error rather than falling back" is **reversed**: 404 now falls back to cargo when available, and errors with a message naming both prerequisites only when cargo is unavailable
- `is_placeholder_formula()`, `install_brew()`, `install_scoop()`, and the `InstallMethod::Brew`/`InstallMethod::Scoop` variants are removed (dead code once brew/scoop leave the decision path)
- The `ddl init` banner and doctor's prerequisites check stop advertising brew/scoop as install channels
- Docs updated: project.md fallback chain, installer module header, README install section, docs/ecosystem-map.md
- incitaciones' npm path is unchanged
- out of scope (deferred, see DDL-ei3 notes): channel-of-record conflict detection in doctor

## Impact

- Affected specs: `bootstrap` (Platform Detection, Init Command, Install Command requirements)
- Affected code: `src/installer.rs` (method selection, fallback, dead-code removal), `src/main.rs` (init banner), `src/diagnostics.rs` (prerequisites check)
- Behavior change: macOS/Windows users with brew/scoop but no rust toolchain are unaffected (binary path needs neither); users without cargo now get binaries where previously they got brew/scoop installs; tools without published release binaries (dont, fabbro today) now require cargo on the machine — previously they also required cargo (brew formulas are placeholders), so no regression
