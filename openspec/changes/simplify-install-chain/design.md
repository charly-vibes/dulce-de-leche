# Design: Simplify Install Chain

## Context

Three decision points currently reference brew/scoop: `best_install_method()` (selection), the `ddl init` banner (reporting), and doctor's `PrerequisitesCheck` (advisory). Removing brew/scoop from selection alone would leave inconsistent reporting; all three change together.

## Goals / Non-Goals

- Goals: one install decision path (binary → cargo), removal of the placeholder-formula special case, identical behavior across platforms for the same tool
- Non-Goals: removing brew/scoop support for *users* (tap formulas stay published); channel-of-record conflict detection (deferred); touching the npm path for incitaciones; changing upgrade semantics (`ddl upgrade` already routes through the same method selection)

## Decisions

### D1. Fallback happens at failure time, not selection time

Whether a release binary exists cannot be known before attempting the download (it depends on the tool's latest release assets). So `best_install_method()` can no longer encode the whole strategy: it returns `Binary` for every non-npm tool, and `install_tool()` handles the fallback when `install_binary` fails with a release-not-found (404) error.

The fallback decision is extracted into a pure, unit-testable function:

```rust
/// Decides the follow-up method when the binary download path fails
/// because no release binary was published for this platform.
pub fn fallback_after_binary_failure(platform: &Platform) -> Option<InstallMethod> {
    if PackageManager::Cargo.is_available() { Some(InstallMethod::Cargo) } else { None }
}
```

Network *transient* failures (timeout, 5xx, DNS) do NOT trigger the cargo fallback — only the definitive "no release asset for this platform" 404/asset-missing case does. Rationale: falling back on transient network errors would silently produce slow cargo compiles (and surprise toolchain requirements) during outages; the definitive-miss case is the only one where cargo is genuinely the only path.

### D2. Distinguishing 404 from other failures

`install_binary` already returns `DdlError::InstallFailed` with distinct messages for "release not found" vs "asset not in archive" vs transport errors. To make fallback routing non-fragile (no string matching), the release-miss case gets its own error variant: `DdlError::NoReleaseBinary { tool, url }`. This is an additive error-enum change; existing variants stay.

### D3. Remove, don't park, the brew/scoop code

Per repo convention (delete unused code, no commented-out remnants): `install_brew`, `install_scoop`, `is_placeholder_formula`, `PackageManager::Brew`/`Scoop` availability checks used only for selection, and `InstallMethod::Brew`/`Scoop` variants are deleted. The `InstallMethod` enum keeps `Binary`, `Cargo`, `Npm`, `Skipped`. If brew/scoop support returns later, git history preserves it.

### D4. Error message when neither path exists

When binary download misses (404) and cargo is unavailable, the error names both remedies explicitly, e.g.:
`⚠ <tool> has no release binary for <platform>, and cargo is not installed. Install Rust (https://rustup.rs) to enable cargo install, or download the binary manually from <repo>/releases.`

### D5. Prerequisites and banner reporting

- `ddl init` banner: reports `binary download (primary) / cargo (fallback: available|not installed)` instead of enumerating brew/scoop
- doctor `PrerequisitesCheck`: keeps `curl`, `git`, `cargo`; drops `brew`, `scoop` (they no longer gate any ddl behavior)

## Risks / Trade-offs

- [Regression] Tools without release binaries now hard-require cargo → mitigated: this is already true today (their brew formulas are placeholders routed away by `is_placeholder_formula`); no behavioral regression
- [Adoption] Machines without rustc lose brew/scoop as automatic channels → mitigated: binary-first covers all five supported platform targets, which is the documented "no prerequisites" promise; cargo fallback is the safety net, not a gate
- [Spec] The 404-no-fallback rule in the current bootstrap spec is intentional design being reversed → called out explicitly in the spec delta so reviewers see the reversal, not just silent drift

## Migration Plan

1. Land code + tests + docs in one change (small surface, no data migration)
2. Manifests are unaffected: `ToolEntry.source` already records the actual channel used per tool ("binary", "cargo", "npm"); old manifests with `"brew"`/`"scoop"` sources remain readable (manifest is schema-tolerant: source is a free string) and `ddl upgrade` will simply re-install via the new policy
3. Rollback: revert the single commit; no persisted state changes

## Open Questions

- None — policy decided by user 2026-09-15 (DDL-ei3). Platform verification (macOS/Windows) happens at implementation time per the 3-platform rule.
