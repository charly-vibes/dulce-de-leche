# Design: add-setup-wizard-onboarding

## Metric definition — TTFIF (time-to-first-initialized-fleet)

Wall-clock seconds from the start of the user's first ddl command in a clean
environment until `ddl status` reports every tool the user selected as
healthy. Two modes, both measured with `scripts/measure-ttfif.sh`:

| Mode | First command | Includes network installs | Measures |
|---|---|---|---|
| `baseline` | `ddl init --yes --human` | yes | current flow |
| `wizard` | `ddl setup` (post-implementation) | yes | proposed flow |

Environment for both: fresh temp `$HOME` + fresh temp repo, PATH reduced to
system defaults plus the pinned toolset build artifacts — the same recipe in
both runs so the comparison is apples-to-apples.

## Baseline (captured 2026-10-20)

Recorded in `baseline.org` next to this design doc, one row per run:
date, mode, machine, ttfif_seconds, tools_healthy, notes.

## Decision gate

Wizard adoption requires, measured with ≥3 runs per mode on the same machine:

1. median wizard TTFIF ≤ median baseline TTFIF, and
2. the wizard path requires strictly fewer manual decisions (prompts answered
   or docs consulted) than the baseline path, counted by the runner.

If (1) fails, park. If (1) passes but (2) fails, re-scope the wizard before
re-measuring — a wizard that costs more time is noise, not onboarding.

## Wizard interaction sketch (post-gate)

1. Detect platform + prerequisites (curl/wget, npm, rustc) — same checks as
   `ddl init`, surfaced as a pre-flight list with human-readable fixes.
2. Ask: fresh config vs migrate existing configs (delegates to `ddl
   migrate`'s dry-run report before applying).
3. Ask: full fleet vs a category subset (core / recommended / extension —
   the registry categories are the taxonomy; see
   `tests/tool_registry_drift.rs` EXPECTED_CATEGORY).
4. Run the standard install + tool-init cascade (the same code paths `ddl
   init` uses — the wizard orchestrates, never reimplements).
5. End with `ddl status` output and `ddl doctor` repo checks; exit non-zero
   with a resume summary if anything failed (same contract as init).
