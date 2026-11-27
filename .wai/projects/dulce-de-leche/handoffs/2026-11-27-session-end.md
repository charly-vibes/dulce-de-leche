---
date: 2026-11-27
project: dulce-de-leche
phase: research
---

# Session Handoff

## What Was Done

- **v0.11.0 released** (repo-status SSOT: `.ddl/status.json`, born doctor-clean gated repos). PRs #118 (SSOT feature + init status-record fix), #119 (release bump/changelog cut), #120 (RustSec url/zip bumps blocking the advisory gate). Tag moved once (first run failed `cargo audit`); final run 46066690952 success, 7 assets, tarballs verified exec-bit, downloaded binary runs.
- **DDL-kk9 closed** — podras FP already fixed upstream (podras-ite 74d0856).
- **DDL-3av closed for ddl** — publish-job chmod verified in the wild; open for wai/dont/espectacular/specodelic/whisper.
- **DDL-8uiv filed + hardened** — the orphan-check pgrep matched any full-cmdline `sleep 60` (foreign leaks / parallel sibling mid-kill); tests serialized + skip-with-reason. Kill logic verified correct.
- DDL-44k noted UNBLOCKED (kk9 closed) — fleet `ddl init --gates` rollout can proceed.

## Key Decisions

- init tail writes the status record advisory-only (DDL-6ox posture), silent in JSON mode.
- Release posture = v0.10.0's: clear RustSec blockers, chrono-unmaintained stays the one allowed warning; tag force-moved pre-publication (release never created on the failed run).

## Gotchas & Surprises

- `cargo update -p url` no-ops — security bumps need `--precise`.
- `gh run watch --exit-status | tail -N` masks the failure exit code (PIPESTATUS).
- testaruda pre-push selection widens on Cargo.lock changes — host-env-failing tests then block every push.
- DDL-3av's "fix committed on branch in all 9 repos" was false for ddl: the branch is gone; main already had the chmod.

## What Took Longer Than Expected

- The doctor-clean CI failure: local runs skip the contract (unpinned wai), so CI was the only honest oracle; verification of the fix needed a manual init→doctor repro bypassing the skip gate.

## Open Questions

- None blocking.

## Next Steps

1. DDL-44k: fleet `ddl init --gates` rollout (now unblocked) — verify each repo with `ddl doctor --json`.
2. DDL-3av: re-verify/land exec-bit fixes in wai/dont/espectacular/specodelic/whisper.
3. P1s: DDL-arb (vampiro CI red), DDL-cfd (README front-door).

## Context

### open_issues

```
○ DDL-3et P1 Research: Hegel property-based testing plan (antithesis-research)
○ DDL-44k P1 fleet: re-run ddl init --gates so every ddl-wired repo adopts the armed pre-commit gate (DDL-sl9)
○ DDL-76lm P1 [epic] Epic: make repo-status SSOT self-maintaining (session-mined frictions)
○ DDL-7a7 P1 Implement Hegel property tests
○ DDL-9gso P1 [bug] [bug] gently: pre-push cargo-gate is RED on clippy deny-warnings debt already on main - it blocks EVERY push (incl. the beads export)
○ DDL-9kz P1 [epic] Fleet close-loop: make the session-close protocol visible to every harness (60-day session retrospective)
├── ○ DDL-9kz.5 P2 ddl fleet report: ship the session-telemetry pipeline (pi + omp + codex) as a ddl command
├── ○ DDL-9kz.8 P2 Fleet: resolve one canonical skill root for every harness (agents guess ~/.pi/agent/skills, real root is ~/.agents/skills)
├── ○ DDL-9kz.9 P2 wai CLI ergonomics (cross-repo: charly-vibes/wai) - split into 4 child tickets
│   ├── ○ DDL-9kz.9.1 P2 wai: fix 'wai feedback --from-last-error' (it must not resolve the error text as an artifact/filename)
│   ├── ○ DDL-9kz.9.2 P2 wai: infer the project from cwd instead of erroring 'Multiple projects found'
│   ├── ○ DDL-9kz.9.3 P2 wai: 'did you mean' suggestions for unknown flags
│   └── ○ DDL-9kz.9.4 P2 harness (omp): todo/task tool errors name the accepted forms (NOT a wai command - 'wai todo' does not exist)
└── ○ DDL-9kz.10 P2 DESIGN: choose how to reduce pi edit exact-match friction (8.2% of edit calls fail)
○ DDL-arb P1 vampiro main CI red: openspec validate --all exits 1 in dogfood smoke
○ DDL-cfd P1 docs: apply README front-door standard v0.4 (§5.1)
○ DDL-cfxh P1 [bug] [bug] gently: pre-commit specodelic/spk-lint gate is RED on a pre-existing spec Reference-Typing violation - it blocks EVERY commit, including the beads export
○ DDL-01q P2 [bug] Test flakes under parallel load: init/gates integration tests fail non-deterministically (test_init_json_output, gates_is_idempotent_on_rerun, test_init_yes_creates_ddl_dir each failed a different full `cargo test` run on 2026-10-16 during the v0.9.0 release; all pass in isolation). Likely shared-state or resource contention — a release test job rerun-risk. Diagnose and de-flake (serial_test or per-test temp isolation).
○ DDL-3av P2 [bug] Fleet-wide: release tarballs ship binaries without the exec bit — curl | tar xz && ./tool fails
○ DDL-85p P2 Standard: testaruda-backed just test recipes across the DDL repos
○ DDL-9kz.11.1 P2 docs: fleet-incidents entry - 'session-close protocol invisible to non-pi harnesses'
○ DDL-9kz.11.2 P2 docs: standardization.md section 10 - the session-close protocol must be repo-level (harness-agnostic)
○ DDL-9x4 P2 P1 epic: fleet provenance.json rollout — make fail-closed installs actually pass (DDL-yts/wof completion)
└── ○ DDL-9x4.11 P2 DESIGN: external-tool provenance gap — bd (gastownhall) cannot publish our provenance.json
○ DDL-api P2 [epic] feat(pretender): index-accurate gate - scan the staged blob, not the worktree copy
├── ○ DDL-fjam P2 feat(pretender): read the staged blob instead of the working-tree copy
├── ○ DDL-qxji P2 design(pretender): decide the semantics for an index-accurate gate scan
└── ○ DDL-wstc P2 feat(pretender): CLI surface for index-accurate scanning
○ DDL-gzak P2 gates: migrate src/gates/lefthook.rs onto genesis::git_hooks
○ DDL-js72 P2 ddl: canonical bd-semantics AGENTS.md block + doctor check for contradictory claims
○ DDL-myf P2 Fleet sweep: migrate the nine legacy openspec repos (openspec update, project.md → config.yaml)
○ DDL-qfc5 P2 Decide AGENT-SUMMARY.md convention: manage it or archive it to .wai
○ DDL-t5r P2 ddl: manage just as a DDL tool + doctor gate for the DDL-85p test-runner recipes
○ DDL-59k P3 chore(wai): artifact hygiene - drop the duplicate RO5U review copy and add a cleanup-round research artifact
○ DDL-6gg P3 chore(wai): doctor warnings - missing tdd-ro5-gate omp rule and projects without .state
○ DDL-6gq P3 [bug] ddl init invokes a stale wai and re-injects a duplicate ah managed block
○ DDL-7ag P3 Replicate dont CI speedup pattern (rust-cache + cargo-binstall + nightly coverage) across charly-vibes repos
○ DDL-8uiv P3 hanging_child_with_descendants_killed_within_deadline_plus_one fails deterministically on workstation
○ DDL-bij8 P3 chore(wai): add the cleanup-round research artifact
○ DDL-g6w P3 Adoption/UX reframe: reduce onboarding friction (setup-wizard entry point experiment)
○ DDL-jjox P3 upstream brief-drift HEAD-churn normalization to charly-vibes/podras
○ DDL-ljk2 P3 chore(wai): initialize the missing .state for the dulce-de-leche and security-remediation projects
○ DDL-ndn P3 [bug] dont release.yml crates.io publish fails on dirty-worktree check (11 uncommitted generated files)

--------------------------------------------------------------------------------
Total: 43 issues (43 open, 0 in progress)

Status: ○ open  ◐ in_progress  ● blocked  ✓ closed  ❄ deferred
Priority: P0–P4 (label only; not a status icon)
```

