---
tags: [pipeline-run:tdd-ro5-2026-11-25-ddl-sl9, pipeline-step:orient]
---

ORIENT: DDL-sl9 [P1 bug] managed pre-commit pretender gate is a no-op — src/gates/lefthook.rs PRE_COMMIT_BLOCK renders 'pretender check --diff-only --mode gate'; --diff-only diffs the unstaged tree, which is empty at pre-commit time (everything is staged), so the gate never scans a file. pretender check --staged exists ('Only check git-staged files', mutually exclusive with --diff-only). Pre-push gate keeps --diff-only (changes are committed at push time, not staged). Scope: change the pre-commit default only; keep .ddl/config.toml gates.pretender_gate override working; update the three test assertions (src/gates/tests.rs:100,449; tests/gates_command.rs:81). No openspec spec encodes the command string (rg over openspec/ + docs/ found none), so no spec change needed.
