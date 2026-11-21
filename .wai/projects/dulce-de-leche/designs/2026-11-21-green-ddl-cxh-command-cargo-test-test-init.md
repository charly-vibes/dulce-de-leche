---
tags: [pipeline-run:tdd-ro5-2026-11-21-ddl-cxh, pipeline-step:green]
---

GREEN (DDL-cxh) — command=`cargo test --test init_cascade` → 19 passed, 0 failed.
- Product fix (src/commands/init/phases.rs, init_cascade): before the ordered loop, if any candidate resolves (alias-safe, via platform::resolve_tool) to tool "ah", create_dir_all("openspec") in cwd; failure logs a warning and lets ah init fail with its own clear diagnostic instead of aborting the cascade. Idempotent; never touches an existing openspec/.
- Decision: scaffold the dir rather than injecting the openspec tool-init as a hidden prerequisite — no new dependency on the openspec binary being installed, no ordering surprise for --tools scoping; the canonical openspec-before-ah ordering (DDL-ehu/gates) remains authoritative when openspec IS in scope. One gate, no new abstraction (per out-of-scope).
- Also green: cargo test --test init_canonical (8), --lib platform:: (6), cargo fmt --check, cargo clippy -D warnings.

