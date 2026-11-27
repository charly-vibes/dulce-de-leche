# Design — ddl-status-single-source-of-truth

## Context

See proposal.md — Why. Current state that shapes the approach:

- The podras brief envelope already proves the pattern this change generalizes:
  typed JSON with `generated_at` + per-source `observed_at`/`staleness` fields
  (`BRIEF.json`, schema `brief.v1`).
- ddl already talks to every managed tool over a subprocess protocol
  (`src/platform/tool.rs` descriptors; `wai status --json` parsing in
  `src/diagnostics.rs`); genesis `git` helpers exist for repo state.
- `.ddl/` is the managed repo directory (dot-ddl spec: directory layout, gitignore
  convention, file locking) — a new record file belongs there.
- Gate emission machinery exists: `ddl init --gates` writes managed blocks
  (`src/installer/init.rs::agents_block`, `ddl.repo.blocks` doctor check, lefthook
  templates). The brief-drift gate itself already exists as a hand-rolled pre-commit
  hook in exactly two repos (dulce-de-leche, podras lefthook.yml) — it needs to become
  a ddl-emitted template, not stay a copy-paste convention.
- Evidence base: `.wai/projects/dulce-de-leche/research/2026-11-27-status-ssot-friction.md`
  (Rule-of-5 reviewed; the measurement-method corrections from that review — diff-based
  staleness, not last-touch dates — are binding for this design).

## Goals / Non-Goals

**Goals:**
- One owned answer to "what is this repo's status": ddl maintains it, obtains the feeds.
- Freshness is data, computed from content state, visible in every output.
- Kill the per-agent re-derivation of the render/export ordering.
- Doctor checks guard the pipeline and the semantics statements in AGENTS.md.

**Non-Goals:**
- No changes to how bd stores data (no-db stays); no podras/bd/wai internal changes
  beyond what their existing CLIs already emit (DDL-kk9 is a prerequisite ticket in
  podras, not here).
- No removal of any tool's own status command; ddl reads them.
- No new package resolution or launcher behavior.
- Not migrating `AGENT-SUMMARY.md` content automatically — ddl reports it as unmanaged;
  adoption/archival is a human decision (DDL-qfc5).

## Decisions

### D1 — The record is tracked; freshness comes from re-obtainment, not trust

`.ddl/status.json` is committed (like `BRIEF.json` today) so a freshly cloned repo or a
CI job has orientation with zero tools installed. But it is a *contract with an expiry*:
`ddl status` re-obtains nothing by default, shows the record with staleness flags, and
`ddl status --refresh` re-runs the obtainers. The doctor check `ddl.repo.status-fresh`
fails when the record's feeds are stale by content-state comparison.

- Alternative rejected: untracked, always re-obtain. Loses offline/CI orientation and
  recreates the "live command fails in unconfigured repos" class (F-C evidence).
- Alternative rejected: trust `generated_at` alone. That is the exact failure mode the
  research found (stale tracked renders trusted as current).

### D2 — Staleness is computed from content state, never file timestamps

For each feed, staleness compares committed artifact vs a fresh re-obtainment:

- brief feed: committed `BRIEF.json` vs a fresh `podras brief --json` envelope diff.
- beads feed: tracked `.beads/issues.jsonl` vs `bd export` to a temp file (diff).
- git feed: computed live, always `fresh` (cheap, no committed artifact).

Last-touch timestamps appear only as display metadata. This encodes the Rule-of-5
correction (EDGE-001: the proxy mislabeled whisper).

### D3 — Feeds are descriptors on the tool table, obtainers behind a trait

`src/platform/tool.rs` gains a per-tool `status_feed` capability (descriptor field,
like the existing status/doctor descriptors): podras → brief envelope, bd → issue
counts + export diff, openspec → active changes, git → repo state (genesis helpers).
A `FeedObtainer` trait (subprocess protocol, per-tool timeout, error capture) keeps
ddl orchestrator-clean: no library linking, release cycles stay decoupled.

### D4 — `--refresh` runs the write ordering with the existing file lock

`ddl status --refresh` reuses dot-ddl's file locking (Requirement: File Locking) and
executes: re-obtain feeds → regenerate brief render (when podras + brief exist) →
`bd export` to the tracked path → rewrite record. Steps are individually reported; a
failed step names the producer's error and the remaining command (spec: Render ordering
failure is actionable). The same ordering is what the emitted `brief-drift` gate's
failure message prescribes — one source of truth for the ordering (answers the 3-retry
loop and the two-agents-racing edge case; the gate message and `--refresh` can't drift).

### D5 — Doctor repo checks are a namespace, not a tool

`ddl.repo.*` checks live beside tool checks in the doctor report but under a separate
`repo` JSON key and their own section in human output, so repo findings can't drown
tool findings (REPLy.jl #46 evidence: `pass: 0, warn: 29` noise). Checks:
`status-fresh` (D2 comparison), `beads-block` (parse AGENTS.md beads block; fail on
"source of truth" applied to the export), `brief-drift-gate` (gate present where render
committed), `unmanaged-status-artifacts` (root-level hand-written status files).

### D6 — Gate emission extends the existing installer gates

The brief-drift gate template is emitted by `ddl init --gates` into the repo's
lefthook config for any repo with a committed brief render; beads-only repos get the
existing export-drift guard instead (spec: No render, no gate). Fleet rollout rides
DDL-44k (re-run `ddl init --gates` across repos). The two hand-rolled gates (dulce,
podras) are migrated to the template so the convention stops forking.

### D7 — Record schema is versioned and tolerant

`repo-status.v1` mirrors the brief envelope's shape (`ok`/`data` envelope, per-feed
entries, `schema_version`). Unknown feed fields are preserved, not rejected; a feed
schema bump (e.g. `brief.v2`) is recorded with its version rather than breaking the
refresh. Same error-identity discipline as `cli-core` JSON output: `--json` and human
outputs name the same error codes (wai-4hqy lesson).

## Risks / Trade-offs

- **Another writer racing the gate.** Two agents refreshing concurrently: mitigated by
  the dot-ddl lock (D4) and the gate message stating the ordering (spec). Residual
  risk accepted; no distributed lock.
- **Obtainment latency.** `--refresh` runs 4 subprocesses (podras, bd, git, openspec);
  each bounded by the per-tool timeout. `ddl status` without `--refresh` stays fast —
  it reads the record.
- **Feed schema drift** across tool versions: tolerated by D7; a feed that fails to
  parse is `unavailable` with the raw error attached, never silently dropped.
- **Adoption lag.** Until DDL-44k's re-run, most repos won't have the gate or the
  record; `ddl status`'s "no status record" path (health spec) must be a helpful
  suggestion, not an error.

## Migration Plan

1. Land DDL-kk9 (podras parser FP) — prerequisite, other repo.
2. Ship `src/repo_status.rs` (record + obtainers + staleness) with the health-spec
   deltas; `ddl status` degrades gracefully without a record (done).
3. Emit the brief-drift gate template + AGENTS.md beads block via `ddl init --gates`;
   migrate dulce/podras hand-rolled gates to the template.
4. Fleet rollout via DDL-44k; `ddl doctor` repo checks flag stragglers.
5. Close out DDL-852c/DDL-4jvz as implemented by this change; DDL-js72/DDL-qfc5 land
   as the beads-block check and the unmanaged-artifacts check respectively.
