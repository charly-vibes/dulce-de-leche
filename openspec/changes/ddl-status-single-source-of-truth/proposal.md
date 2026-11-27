# Proposal

## Why

Every repo in the charly-vibes fleet answers "what is this repo's status?" with a different
artifact — podras `BRIEF.md`/`BRIEF.json`, bd `.beads/issues.jsonl`, hand-written
`AGENT-SUMMARY.md`, `wai status`, openspec state — and every one of them is a stale cache
that nothing owns (measured 2026-11-27: brief renders 7–9 days staler than the tracker in
7 of 10 audited repos; the render-drift gate armed in only 2 of 10; bd storage semantics
contradict across repos' AGENTS.md blocks). Agents pay a reconciliation tax on every
orientation and, when nothing is trusted, invent their own status stores (session-mined
evidence: `.wai/projects/dulce-de-leche/research/2026-11-27-status-ssot-friction.md`).

Owner decision 2026-11-27 (confirming the Rule-of-5 review of that research): **ddl is the
unique source of truth for repo status, and it manages the obtention of that status from
every other tool.** No other artifact is the contract; renders, exports, and summaries
become outputs of ddl's pipeline or feeds it reads.

## What Changes

- **New: repo-status store owned by ddl.** ddl maintains a per-repo status record
  (`.ddl/status.json`, schema `repo-status.v1`) that it *obtains* from the ecosystem's
  status producers via the subprocess protocol: podras `brief --json` envelope (intent,
  in-flight, staleness), bd open/blocked counts, git branch/sync/dirtiness, openspec
  active changes. ddl refreshes the record on demand (`ddl status`) and on `ddl init`.
- **New: `ddl status` project-status section.** `ddl status` answers both halves of the
  question — *tool* health (existing) and *repo* status (new: what's in flight, tracker
  counts, staleness flags) — from the record it owns, instead of leaving agents to run
  four commands and reconcile them.
- **New: freshness as first-class data.** Every feed ddl obtains carries an
  `observed_at` + `staleness` field (pattern already proven by the podras brief
  envelope); the record exposes them; stale feeds are visible, never silent.
- **Modified: doctor checks guard the pipeline.** New repo checks
  (`ddl.repo.status-fresh`, `ddl.repo.beads-block`, `ddl.repo.brief-drift-gate`) verify
  the record is fresh, that AGENTS.md beads blocks state the correct storage semantics
  ("issues.jsonl is the committed export; git is the sync; the live DB is per-machine"),
  and that the render-freshness gate is armed where renders exist.
- **Modified: the render loop becomes ddl's write path.** Where a podras brief render
  exists, `ddl status --refresh` runs the obtain-and-write ordering
  (bd write → `podras brief --write` → `bd export` → record refresh) that today agents
  re-derive by trial and error (3 commit retries in one 2026-11-25 session).
- **New: `ddl.repo` doctor subcommand namespace** for these checks so `ddl doctor` output
  separates tool health from repo-status health.

Out of scope (per proposal rules): no package resolution changes, no unified launcher,
no change to how bd stores data internally, no migration of podras/bd/wai internals,
no removal of any managed tool's own status command (ddl reads them; it does not
replace them).

## Capabilities

### New Capabilities
- `repo-status`: ddl-owned single source of truth for repo status — the status record it
  maintains, the feeds it obtains (podras brief envelope, bd, git, openspec), freshness
  semantics, the `--refresh` obtain-and-write ordering, and what renders/exports are
  outputs of this pipeline.

### Modified Capabilities
- `health`: `ddl status` gains a project-status section (repo status next to tool
  health); `ddl doctor` gains the `ddl.repo.*` checks and separates repo-status findings
  from tool findings in its report and JSON.

## Impact

- **Affected specs:** new `specs/repo-status/spec.md`; delta on `openspec/specs/health/spec.md`
  (Status Command + Doctor Command requirements).
- **Affected code:** `src/platform/tool.rs` (podras descriptor — status/refresh seams),
  `src/diagnostics.rs` (status/doctor report builders), new `src/repo_status.rs`
  (feed obtainers + record writer + schema), `src/installer/init.rs` (gate emission),
  `justfile`/lefthook templates for the brief-drift gate.
- **Protocol:** subprocess only (podras `brief --json`, `bd export`/`bd list --json`,
  `git`); no shared-library linking, consistent with the existing architecture.
- **Dependencies:** DDL-kk9 (podras malformed-line false positive) must land first —
  a status SSOT built on a lying observer moves the reconciliation cost. Rollout vehicle:
  DDL-44k (`ddl init --gates` re-run fleet-wide).
- **Fleet tickets:** re-scopes DDL-852c (project-status section) and DDL-4jvz
  (brief-drift gate rollout) under epic DDL-76lm; DDL-js72 (canonical bd-semantics
  block) and DDL-qfc5 (AGENT-SUMMARY.md convention) become requirements here.
