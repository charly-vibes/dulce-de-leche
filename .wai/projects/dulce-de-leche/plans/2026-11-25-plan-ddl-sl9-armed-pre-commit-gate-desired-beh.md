---
tags: [pipeline-run:tdd-ro5-2026-11-25-ddl-sl9, pipeline-step:plan]
---

PLAN DDL-sl9 — armed pre-commit gate.

Desired behavior: the ddl-managed pre-commit pretender gate must inspect the files the commit actually contains. Rendered default becomes 'pretender check --staged --mode gate' (pretender: 'Only check git-staged files', mutually exclusive with --diff-only). Proven in a throwaway repo: a 62-line staged function exits 1 and names the file under --staged, while --diff-only (unstaged diff, empty after git add) exits 0.

Out of scope: the pre-push gate (keeps --diff-only — at push time changes are committed, not staged); changing the .ddl/config.toml gates.pretender_gate override mechanism; touching pretender itself; fleet rollout (each repo re-runs 'ddl init --gates').

Test cases (RED first):
1. src/gates/tests.rs:100 — init in a fresh repo renders the pre-commit block with 'run: pretender check --staged --mode gate'.
2. src/gates/tests.rs:449 — render_pre_commit_block emits the staged command.
3. tests/gates_command.rs:81 — gates_wires_lefthook_gitignore_and_beads_in_fresh_repo asserts the staged command and that --diff-only is gone from the pre-commit block.
4. Regression: the pre-push block still uses --diff-only.

Narrow test command: cargo test --test gates_command && cargo test --lib gates
Full verification command: just fmt-check && just lint && just test-full
Files touched: src/gates/lefthook.rs (PRE_COMMIT_BLOCK + DEFAULT_PRETENDER_GATE), src/gates/tests.rs, tests/gates_command.rs, plus this repo's own lefthook.yml managed block if present (it is not — this repo uses bd/fmt/lint hooks).
