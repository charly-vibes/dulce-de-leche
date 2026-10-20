# Design: add-setup-wizard-onboarding

## Metric definition — TTFIF (time-to-first-initialized-fleet)

Wall-clock seconds from the start of the user's first ddl command in a clean
environment until `ddl status` reports every tool the user selected as
healthy. Two modes, both measured with `scripts/measure-ttfif.sh`:

| Mode | First command | Includes network installs | Measures |
|---|---|---|---|
| `baseline` | `ddl init --yes --human` | yes | current flow |
| `wizard` | `ddl setup` (post-implementation) | yes | proposed flow |

Environment for both: fresh temp `$HOME` + fresh temp repo, PATH stripped of
preinstalled fleet tools (a real new user has none) — the same recipe in
both runs so the comparison is apples-to-apples.

### Subset amendment (2026-10-20, after baseline capture)

The full-fleet metric is **unattainable on this platform**: funkalization and
wild have no published release binaries and no cargo fallback (baseline run,
`init` exit 1, partial failure — see baseline.org). The gate metric is
therefore:

- **`ttfif_core`** — wall-clock until `ddl status` reports healthy every
  tool with a published binary on the measurement platform (core +
  recommended: wai, testaruda, turu, dont, ah). This is the gate metric.
- Full-fleet TTFIF is recorded as a secondary metric whenever it is
  attainable; revisiting it is deliberate, not automatic.

Gate scope limitation: measurements are same-machine A/B comparisons on one
platform (Linux x86_64 today). The fleet is three-platform; extending the
comparison to macOS/Windows is a follow-up, not part of this gate.

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
