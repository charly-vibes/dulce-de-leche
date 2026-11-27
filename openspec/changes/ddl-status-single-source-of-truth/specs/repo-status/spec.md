# Spec Delta — repo-status (new capability)

## Purpose

ddl is the single source of truth for a repository's status: it maintains a status record
it obtains from the ecosystem's status producers (podras, bd, git, openspec) and answers
"what is this repo's status — and is that answer fresh?" for humans and agents.

## ADDED Requirements

### Requirement: Status Record

The CLI SHALL maintain, per repository, a status record at `.ddl/status.json` with schema
`repo-status.v1` that it obtains from the managed tools' own commands via the subprocess
protocol.

#### Scenario: Record created by refresh

- **WHEN** user runs `ddl status --refresh` in a repo with `.ddl/manifest.json`
- **THEN** the system obtains each configured feed and writes `.ddl/status.json`
- **AND** the record contains `generated_at`, and one entry per feed with
  `name`, `observed_at`, `staleness`, and the feed's status payload
- **AND** the record is valid JSON parseable without ddl

#### Scenario: Feeds obtained via subprocess

- **WHEN** the system refreshes the status record
- **THEN** it obtains the podras brief envelope (`podras brief --json`) when podras is
  installed and the repo has a brief, bd issue counts (`bd list --json`), git state
  (branch, ahead/behind, dirty file count), and openspec active changes
- **AND** a feed whose producer is absent or fails is recorded as `unavailable` with the
  producer's error output, never as missing data

#### Scenario: Feed producers may be absent

- **WHEN** the system refreshes in a repo where podras, bd, or openspec is not installed
  or not configured for that repo
- **THEN** the system records those feeds as `unavailable` with reason
- **AND** still writes a valid record with the remaining feeds
- **AND** exits 0 (absence of a feed is a fact about the repo, not a failure)

### Requirement: Freshness is first-class

Every obtained feed entry in the status record SHALL carry `observed_at` and a
`staleness` classification, and the record SHALL expose the age of each feed.

#### Scenario: Stale feed is visible

- **WHEN** a feed's `observed_at` is older than the last change to that feed's
  underlying store (for example, the committed brief render older than
  `.beads/issues.jsonl`'s last commit)
- **THEN** the feed's `staleness` is `stale`
- **AND** the staleness verdict is computed by comparing content state, not by
  file last-touch timestamps alone (export diff over `.beads/issues.jsonl`, not mtime)

#### Scenario: Freshness exposed in output

- **WHEN** user runs `ddl status --json`
- **THEN** the project-status data includes each feed's `observed_at` and `staleness`
- **AND** staleness never changes the exit code silently (it is data, reported)

### Requirement: Obtain-and-write ordering

The CLI SHALL provide `ddl status --refresh` that performs the full status pipeline in a
single ordering so agents never re-derive it: write-side updates (bd issue changes) are
followed by render regeneration (`podras brief --write`) where a brief exists, then
tracked export update (`bd export` to `.beads/issues.jsonl`), then record refresh.

#### Scenario: Refresh regenerates the render before exporting

- **WHEN** user runs `ddl status --refresh` in a repo with a podras brief and beads
- **THEN** the system runs the ordering: bd-side writes are already done by the caller,
  the brief render is regenerated, the tracked export is updated, the record is refreshed
- **AND** a pre-commit `brief-drift` check passes after the ordering without a manual
  retry loop

#### Scenario: Render ordering failure is actionable

- **WHEN** a step of the ordering fails (for example, `podras brief --write` errors)
- **THEN** the system reports which step failed, the producer's error output, and the
  exact remaining command to run
- **AND** does not continue to later steps with stale inputs

### Requirement: Renders and exports are outputs, not the contract

The committed podras brief render (`BRIEF.md`/`BRIEF.json`), the tracked beads export
(`.beads/issues.jsonl`), and any session summary artifact SHALL be treated as outputs of
(or inputs to) ddl's status pipeline, never as the repository's status contract; the
status record is the contract.

#### Scenario: Contract precedence documented in-band

- **WHEN** ddl emits a status report or refreshes a render
- **THEN** the emitted output identifies ddl as the producer with a `generated_at`
  timestamp and names the live record as the source of truth

#### Scenario: Session summary artifacts

- **WHEN** a repo contains a hand-written status artifact (`AGENT-SUMMARY.md` or
  equivalent) with no producer
- **THEN** `ddl status` reports it as an unmanaged status artifact with its last commit
  date
- **AND** suggests either archiving it out of the repo root or adopting it into the
  managed pipeline

### Requirement: Gate arming for render freshness

The CLI SHALL, via `ddl init --gates`, arm a pre-commit render-freshness gate in any repo
where a podras brief render is committed, failing the commit when the committed render
differs from a fresh render.

#### Scenario: Gate armed by init

- **WHEN** `ddl init --gates` runs in a repo with a committed brief render
- **THEN** the pre-commit gate that compares the committed render against a fresh render
  is present and active
- **AND** the gate's failure message states the required ordering (refresh render,
  update export, re-commit) instead of only reporting drift

#### Scenario: No render, no gate

- **WHEN** `ddl init --gates` runs in a beads repo without a podras brief
- **THEN** the system does not arm a render gate for that repo
- **AND** export drift is covered by the existing export-drift guard instead

### Requirement: Canonical beads-semantics statement

The CLI SHALL, via `ddl init`, write an AGENTS.md beads block whose storage-semantics
statement is: the live database is per-machine, `.beads/issues.jsonl` is the committed
export, and git is the sync mechanism; and `ddl doctor` SHALL flag any repo whose beads
block asserts the export file itself is the source of truth.

#### Scenario: Contradictory block flagged

- **WHEN** `ddl doctor` runs in a repo whose AGENTS.md beads block claims
  `.beads/issues.jsonl` is the source of truth
- **THEN** the doctor reports a repo check failure naming the file and the incorrect
  claim
- **AND** suggests re-running `ddl init --gates` to rewrite the managed block

#### Scenario: Correct block not flagged

- **WHEN** `ddl doctor` runs in a repo whose beads block states the export/sync semantics
- **THEN** the repo check passes
