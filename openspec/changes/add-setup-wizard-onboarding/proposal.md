# Proposal: add-setup-wizard-onboarding

## Why

Cross-provider corpus evaluation (2026-10-14 window, grounded against the
current implementation 2026-10-20) converged on one strategic finding: ddl
delivers epistemic-discipline value but is gated behind high cognitive load
(~a dozen independent CLIs, one orchestration surface), and external adoption
is zero (0 stars, 0 forks). Evaluators also noted no quantitative benefit
data beyond dogfooding. A new user cannot answer "how do I get from zero to
a working fleet?" without reading docs.

The fleet's own install surface already handles the *mechanics* (`ddl init`
is one command). What is missing is the *guided* entry: a new user facing a
prompt they don't understand, with no notion of what init will touch, which
tools they actually need, or how to verify success.

## What Changes

- Add a `ddl setup` wizard mode: an interactive, step-by-step entry point
  (install → migrate decision → tool selection → gates → verify) that ends
  with a doctor-clean repo and a printed summary of what it did.
- Keep every existing non-interactive path byte-for-byte intact: `ddl init
  --yes`, CI flows, and agent flows are unaffected (drift-tested).
- The wizard is an entry-point layer only — it must not collapse or hide the
  epistemic-discipline tooling (per ticket ground rule).

## Measurement Gate (hard requirement)

**The wizard ships only if measured better.** Metric: time-to-first-
initialized-fleet (TTFIF) — wall-clock from a new user's first command to
`ddl status` reporting all selected tools healthy in a clean environment.

- Baseline: `scripts/measure-ttfif.sh` captures the current flow.
- Comparison: wizard flow captured with the same script in wizard mode.
- Adoption criterion: wizard TTFIF ≤ baseline TTFIF **and** fewer required
  decisions/lookups for a first-time user. If not better, the wizard is
  parked (record the result in this change; do not implement to ship).

## Impact

- Affected specs: `specs/setup-wizard` (new capability)
- Affected code: `src/main.rs` (new subcommand), `src/cli.rs`, a new
  `src/setup_wizard.rs` module; no changes to installer/gates internals
- Non-goals: no changes to manifest format, install protocol, or gates
  wiring; no new install sources
