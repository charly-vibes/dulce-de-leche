---
tags: [pipeline-run:tdd-ro5-2026-11-25-ddl-sl9, pipeline-step:fix-review]
---

RO5U-FIXES: both findings fixed. Low (tests/gates_command.rs:78-84) — the assertion comment now keeps the DDL-3fd pointer ('--staged ... keeps DDL-3fd's intent: a commit that does not touch files carrying pre-existing violations is not blocked'), so the reason the new flag preserves the old gate's intent stays discoverable next to the assertion instead of only in a doc comment in another file. Medium (src/gates/lefthook.rs:30 fleet rollout untracked) — tracked as beads DDL-44k (P1): 'fleet: re-run ddl init --gates so every ddl-wired repo adopts the armed pre-commit gate (DDL-sl9)', with the self-healing mechanism (src/gates/detect.rs flags the drifted span and ddl init --gates repairs it) recorded in the ticket body. Re-verified after the fixes: cargo test --lib gates -> 29 passed / 0 failed; cargo test --test gates_command -> 3 passed / 0 failed; just fmt-check clean; just lint 0 warnings.
