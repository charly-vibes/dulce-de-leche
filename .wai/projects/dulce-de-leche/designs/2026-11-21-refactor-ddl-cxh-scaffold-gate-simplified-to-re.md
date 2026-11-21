---
tags: [pipeline-run:tdd-ro5-2026-11-21-ddl-cxh, pipeline-step:refactor]
---

REFACTOR (DDL-cxh): scaffold gate simplified to resolve_tool(...).is_ok_and(|tool| tool.name == "ah") — drops map/unwrap_or(false); identical behavior, clippy/fmt clean, init_cascade 19/19 green. No other duplication found in scope. Out-of-scope wart noted, not touched: ddl_cmd() calls common::clean_git_env twice (pre-existing; harmless idempotent call) — candidate for a future tidy ticket.

