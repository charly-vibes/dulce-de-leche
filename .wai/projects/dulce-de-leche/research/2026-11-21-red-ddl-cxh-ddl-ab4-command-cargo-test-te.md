---
tags: [pipeline-run:tdd-ro5-2026-11-21-ddl-cxh, pipeline-step:red]
---

RED (DDL-cxh + DDL-ab4) — command=`cargo test --test init_cascade`
- DDL-ab4 red (pre-existing, evidence recorded earlier): 7 tests red on dev machine with populated ~/.ddl/bin; after the ddl_cmd HOME-sandbox (test-only edit) all 18 pre-existing tests GREEN. Sandbox verified in child env: HOME=/tmp/.tmp.../home, stub-bin first, no ~/.ddl/bin prepend leak.
- DDL-cxh red (new): `espectacular_init_scaffolds_missing_openspec_dir` FAILED for the expected missing behavior — stub ah exits 1 ([ -d openspec ] || exit 1) because ddl does not scaffold openspec/ on --tools espectacular; cascade -> "Error: Partial failure"; assert at tests/init_cascade.rs:436 ("ah initialized" missing). Not setup breakage: T3 (never_clobbers) and T4 (order marker) pass; 18/18 others pass.
- Guards green by design: espectacular_init_never_clobbers_existing_openspec, ah_init_runs_after_openspec_init (marker-tightened).

