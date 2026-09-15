# Tasks: Simplify Install Chain

## 1. Installer core

- [x] 1.1 Add `DdlError::NoReleaseBinary { tool: String, url: String }` variant; return it from `install_binary` for the release-miss / asset-missing cases (replacing the current string-typed `InstallFailed` in those arms)
- [x] 1.2 Rewrite `best_install_method()`: npm tools → `Npm`; everything else → `Binary` on every platform
- [x] 1.3 Add pure fn `fallback_after_binary_failure(cargo_available: bool) -> Option<InstallMethod>` (Cargo when available, else None) — takes a bool, not platform, so tests need no PATH manipulation
- [x] 1.4 In `install_tool()` AND `upgrade_tool()`: on `NoReleaseBinary`, consult 1.3 — fall back to cargo or fail with the both-prerequisites error (D4). Upgrades keep the manifest's recorded channel for cargo/npm (avoids PATH shadowing from mid-life channel switches); legacy brew/scoop records route through the binary chain
- [x] 1.5 Delete dead code: `install_brew`, `install_scoop`, `upgrade_brew`, `upgrade_scoop`, `is_placeholder_formula`, `InstallMethod::Brew`/`Scoop` variants and their `check_prerequisites`/`Display` arms; update the module header doc comment

## 2. Reporting surfaces

- [x] 2.1 `src/main.rs` init banner: replace brew/scoop package-manager report with `binary download (primary) / cargo fallback: available|not installed`
- [x] 2.2 `src/diagnostics.rs` `PrerequisitesCheck`: drop `brew` and `scoop` from the checked list

## 3. Tests

- [x] 3.1 Unit tests for `best_install_method`: npm tool → Npm; non-npm tool → Binary across all five supported platform triples (`tests/install_policy.rs`, no network)
- [x] 3.2 Unit tests for `fallback_after_binary_failure`: Some(Cargo) when available, None otherwise (bool-injected, no PATH mutation)
- [x] 3.3 End-to-end 404→cargo fallback verified by unit coverage + review instead of a networked integration test: a live test would really `cargo install` and risk overwriting the owner's `~/.cargo/bin` dev builds; the decision functions are fully unit-tested and the wiring is a thin match
- [x] 3.4 Existing test suite green: `just ci` (fmt-check, clippy -D warnings, tests, release build)

## 4. Docs

- [x] 4.1 `openspec/project.md`: fallback chain line → "binary download → cargo install (when no release binary is published and cargo is available)"
- [x] 4.2 README install section: binary-first policy, cargo fallback, brew/scoop as manual-only channels
- [x] 4.3 `docs/ecosystem-map.md`: install-method notes updated (brew/scoop now manual-installs-only)

## 5. Verification & close-out

- [x] 5.1 Manual smoke on Linux: `ddl init --yes --tools turu` in temp dir — banner wording correct, fast-path detection intact
- [ ] 5.2 Flag macOS/Windows verification in DDL-ei3 before close (3-platform rule; CI is linux-only)
- [ ] 5.3 Mark all tasks complete; request review; archive change after approval
