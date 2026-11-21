---
tags: [pipeline-run:tdd-ro5-2026-11-21-ddl-cxh, pipeline-step:orient]
---

# ORIENT DDL-cxh — ah 0.12.3 init contract change breaks ddl init cascade

## Evidence
- `ah init` exits 1 ("openspec/ directory not found") in bare dirs; exits 0 when `openspec/config.yaml` exists. Probed in /tmp, deterministic across 3+ runs (2026-11-18).
- 7 deterministic reds in `tests/init_cascade.rs`: tools_flag_accepts_crate_alias, tools_flag_tolerates_stray_commas, init_children_do_not_inherit_canary_credentials, hanging_init_child_killed_and_budget_identified, hanging_init_child_budget_identified_in_json, failing_tool_init_fails_run_with_resume_summary, failing_tool_init_json_mode_puts_summary_on_stderr.
- Cascade log: "espectacular init failed: Installation failed: ah init exited with code 1: ah: Error: openspec/ directory not found at /tmp/.tmpNNApyX".
- lefthook pre-push gate red on every local push (just test → init_cascade).
- NOT the DDL-01q/xr4 parallel-load flake family (fails in isolation). PR #102 (registry kaya) not the cause — ah version drift is.

## Blast radius
- Real flow broken too: `ddl init --tools espectacular` in any fresh repo without openspec/.
- Fleet repos that dogfood ddl init with ah installed hit the same wall.

## Candidate fixes
(a) ddl cascade scaffolds openspec/config.yaml before the ah init step (orchestrator prepares prerequisites).
(b) Fleet-pin ah to a pre-change version while a compat fix lands.

## Next
Design step: read cascade/tool-step code, dogfood openspec/config.yaml shape, ah docs/skills; define RED test shape (cascade with espectacular succeeds under ah 0.12.3; scaffold is idempotent, never clobbers existing openspec/).

