---
tags: [pipeline-run:tdd-ro5-2026-11-25-ddl-sl9, pipeline-step:quality-ledger]
---

QUALITY LEDGER (DDL-sl9):

Changed:
- src/gates/lefthook.rs — PRE_COMMIT_BLOCK's ddl-pretender-gate entry and DEFAULT_PRETENDER_GATE now render 'pretender check --staged --mode gate' (was --diff-only); doc comments on both constants state the contract and the DDL-sl9 reason.
- src/gates/tests.rs — new test pre_commit_pretender_gate_is_armed_against_staged_files (asserts the rendered block uses --staged, contains no --diff-only, and that DEFAULT_PRETENDER_GATE agrees with the renderer); two existing assertions updated to the staged command.
- tests/gates_command.rs — the fresh-repo wiring assertion expects the staged command, asserts the diff-only form is absent, and keeps the DDL-3fd intent pointer.
- CHANGELOG.md — Unreleased/Changed entry for the armed gate, naming the .ddl/config.toml override path.
- .gitignore — added .testaruda/ (the DDL-managed gitignore set was only half-present: .pretender/ yes, .testaruda/ no), clearing the ddl.repo.gitignore doctor warning.
- .beads/issues.jsonl — DDL-xr4 and DDL-61p closed as duplicates; DDL-44k and DDL-v4s created (export refreshed).

Verified:
- just fmt-check — clean; just lint — clippy -D warnings, 0 warnings.
- cargo test --lib gates -> 29 passed / 0 failed; cargo test --test gates_command -> 3 passed / 0 failed.
- just test-full -> 29 test binaries, 0 failures (run twice).
- just test (affected-set) — green on re-run; the first affected-set run failed once on installer::tests::test_probe_version_at_finds_version_in_ddl_bin, which passes in isolation and is the known parallel-load flake family (DDL-01q/DDL-xr4), not a regression from this change.
- Empirical probe in a throwaway repo: 'pretender check --staged --mode gate' exits 1 and names the staged file; --diff-only inspects nothing at pre-commit time.

Review:
- RO5U status: 5 stages completed, verdict ship-after-fixes. Critical 0, High 0, Medium 1, Low 1 — both fixed (Low in-code; Medium tracked as DDL-44k). Remaining findings: none.

Risks:
- Fleet rollout: consumer repos keep the disarmed --diff-only block until they re-run 'ddl init --gates'; the drift detector flags them meanwhile. Tracked as DDL-44k.
- A repo that hand-edited its managed block now sees a drift warning until it re-runs ddl init --gates (expected self-healing behaviour, same as any default change).
- ddl's own repo still fails 'ddl doctor' (bajan not installed; ah check exits 1 without .espectacular/config.toml, so wiring the managed pre-commit block here would block every commit). Tracked as DDL-v4s.

Next:
- commit the DDL-sl9 change on its own branch, close DDL-sl9 with the ship reason, open the PR (push requires explicit authorization).
