---
tags: [pipeline-run:tdd-ro5-2026-11-21-ddl-cxh, pipeline-step:plan]
---

# Plan DDL-cxh (+ prerequisite DDL-ab4) — ah init needs openspec/; hermetic cascade tests

## Desired behavior
1. (DDL-cxh) `ddl init --tools espectacular` (and default full init) succeeds in a fresh repo
   with real ah ≥0.12: init_cascade ensures the `openspec/` directory exists before ah's init
   runs (ah's contract per espectacular/src/init.rs: `ensure!(repo_root.join("openspec").exists())`
   — dir-only suffices, verified by probe).
2. (DDL-ab4) init_cascade tests are hermetic: `ddl_cmd()` sandboxes HOME to the temp dir so
   `build_tool_init_command`'s `~/.ddl/bin` prepend can never shadow the test stubs.

## Out of scope
- espectacular's own init behavior (separate repo; its ensure! message is fine).
- Per-tool generic prerequisite framework (one edge, one gate — no new abstraction).
- Windows-specific PATH handling; openspec tool pinning (DDL-6ox family handles retries).
- The 4 tests that already self-sandbox HOME (lines 233/275/445/524): redundant after the
  central sandbox but harmless — no churn.

## Test cases (TDD shape)
Commit 1 — DDL-ab4 (test-infra RED already exists: the 7 failing tests ARE the red on any
machine with populated ~/.ddl/bin):
- T1: run `cargo test --test init_cascade` after sandboxing HOME in ddl_cmd() → all 17 green.
- Note: ah_init_runs_after_openspec_init currently passes BY LEAKAGE (real openspec creates
  the dir, real ah then succeeds) — after sandboxing it passes via stubs, as designed.

Commit 2 — DDL-cxh (new RED first, then product fix):
- T2 RED `espectacular_init_scaffolds_missing_openspec_dir`: stub ah with
  `[ -d openspec ] || exit 1; echo 'ah initialized'`, run `ddl init --tools espectacular
  --no-install --human` in a bare temp dir → must succeed (fails today: no scaffold).
- T3 `espectacular_init_never_clobbers_existing_openspec`: pre-create
  `openspec/specs/spec.md` with a marker → run same → success AND marker intact
  (idempotency/no-clobber, repo operations guidance).
- T4 tighten `ah_init_runs_after_openspec_init`: openspec stub writes marker
  `openspec/.ddl-order-probe`; ah stub requires the marker file (not just the dir) so the
  ordering assertion stays meaningful once ddl pre-scaffolds the dir.

## Product fix (GREEN for T2/T3, keep T4 green)
In `src/commands/init/phases.rs` `init_cascade()`: before the ordered loop, if any candidate
resolves (via `resolve_tool`, alias-safe) to tool `ah` → `create_dir_all("openspec")` in cwd
(repo root), logging a warning on failure instead of aborting (ah init then fails with its
own clear message). Idempotent; never touches existing content.

## Files likely touched
- tests/init_cascade.rs (ddl_cmd sandbox; T2/T3/T4)
- src/commands/init/phases.rs (scaffold gate in init_cascade)

## Verification
- Narrow: `cargo test --test init_cascade`
- Full: `just test` (testaruda select/exec loop — the push gate), `just test-full` once
  before PR.

## Tickets/commits
- Claim DDL-cxh + DDL-ab4 (bd update --claim), two commits (one concern each), one PR
  referencing both. Pre-push gate stays green after commit 1 (suite unbroken) — pushes
  unblocked end-to-end.

