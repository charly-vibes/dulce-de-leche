# Repository Status Record

`ddl` is the single source of truth for a repository's status. It maintains a status
record obtained from the ecosystem's status producers (`podras`, `bd`, `git`,
`openspec`) and answers "what is this repo's status — and is that answer fresh?"
for humans and agents. Tool health stays in `ddl status`'s existing tool section;
this page covers the project-status record and the ordering that keeps it fresh.

## The status record

The record lives at `.ddl/status.json` with schema `repo-status.v1` and is committed
like any other tracked state. It is plain JSON, parseable without `ddl`, and mirrors
the podras brief envelope shape:

```json
{
  "ok": true,
  "envelope_version": "0.1",
  "schema_version": "repo-status.v1",
  "generated_at": "2026-11-27T17:00:00Z",
  "data": {
    "feeds": [
      { "name": "podras", "observed_at": "2026-11-27T16:59:58Z",
        "staleness": "fresh", "payload": { "…": "brief envelope data" } },
      { "name": "bd", "observed_at": "2026-11-27T16:59:59Z",
        "staleness": "fresh", "payload": { "open": 33, "blocked": 4 } },
      { "name": "openspec", "status": "unavailable",
        "detail": "no active changes" }
    ]
  }
}
```

Every feed entry carries `name`, `observed_at`, a `staleness` classification, and
either the feed's `payload` or an `unavailable` status with the producer's error in
`detail`. A producer that is absent or fails is recorded as `unavailable` — never as
missing data, and never as a refresh failure: absence of a feed is a fact about the
repo, not an error, so a refresh with some feeds unavailable still exits 0.

## Contract precedence

The status record is the repo-status contract. Everything else is an output of — or
an input to — the pipeline:

| Artifact | Role |
|----------|------|
| `.ddl/status.json` | **Contract** — the source of truth |
| `BRIEF.md` / `BRIEF.json` (committed podras render) | Pipeline output |
| `.beads/issues.jsonl` (`bd export` output) | Pipeline output |
| `AGENT-SUMMARY.md` and similar hand-written files | Unmanaged status artifacts |

Every `ddl status` output identifies `ddl` as the producer with a `generated_at`
timestamp and names the live record at `.ddl/status.json` as the source of truth, so
an agent reading any emitted report can see where to verify. `ddl doctor` reports a
hand-written status artifact at repo root with no producer as an unmanaged status
artifact, with its last commit date, and suggests either archiving it out of the repo
root or adopting it into the managed pipeline.

## Freshness model

Freshness has two tiers:

- **`ddl status` (read-only)** shows the record's age plus a cheap "may be stale"
  heuristic that compares `generated_at` against the last commit touching any feed's
  tracked store file. The heuristic is explicitly labeled — it can mislabel repos
  whose tracker legitimately has not changed.
- **Authoritative verdict** — a content-diff staleness comparison — is computed only
  by `ddl status --refresh` and the `ddl.repo.status-fresh` doctor check.

Staleness compares *content*: the committed `BRIEF.json` payload against a fresh
envelope, and the tracked `.beads/issues.jsonl` against a fresh `bd export` — with
volatile fields (`generated_at`, `as_of`, `cli_version`, `observed_at`) normalized
out. It is never computed from file timestamps alone: two fresh envelopes with
different `generated_at` values are equal after normalization, and a tracker that
has not changed is not stale.

## The obtain-and-write ordering

`ddl status --refresh` performs the full status pipeline in one fixed ordering, under
the `.ddl` file lock, so agents never re-derive it:

1. **Obtain feeds** — `podras brief --json`, `bd list --json`, git state (branch,
   ahead/behind, dirty file count), `openspec list --json`. A producer that is
   absent or fails is recorded `unavailable`; the refresh continues.
2. **Regenerate the render** — `podras brief --write`, where a brief exists.
3. **Update the tracked export** — `bd export` to `.beads/issues.jsonl`.
4. **Rewrite the record** — `.ddl/status.json` is written atomically (temporary
   file, then rename), so a reader never observes a partially written record.

A failing step stops the run: the report names which step failed, the producer's
error output, and the exact remaining command to run. Later steps are never executed
with stale inputs.

Without `.ddl/manifest.json` the repo is not ddl-managed: `--refresh` refuses with
"run `ddl init` first" and exits with code 1, creating no record.

## Doctor repo checks

`ddl doctor` separates repo-status checks from tool diagnostics. Repo checks use the
`ddl.repo.*` namespace, are grouped under a separate `repo` key in `ddl doctor
--json`, and do not change how tool failures are counted:

| Check | Verifies |
|-------|----------|
| `ddl.repo.status-fresh` | The record exists and no feed is stale (authoritative content-diff verdict) |
| `ddl.repo.beads-block` | The AGENTS.md beads-semantics block states the export/sync semantics |
| `ddl.repo.brief-drift-gate` | A render-freshness gate is armed where a brief render is committed |
| `ddl.repo.unmanaged-status-artifacts` | Warns on hand-written status artifacts (`AGENT-SUMMARY.md` class) |

`ddl doctor --fix` refreshes a stale or missing record via the obtain-and-write
ordering and reports the refresh as an applied fix.

## Brief-drift gate

`ddl init --gates` arms a pre-commit render-freshness gate in any repo where a podras
brief render is committed. The gate fails the commit when the committed render
differs from a fresh render. Its failure message:

- names the required ordering (refresh the render, update the export, re-commit)
  instead of only reporting drift, and
- includes the committed envelope's producing `cli_version` — a podras version bump
  between render and check is self-healing (re-running the ordering regenerates the
  render with the installed version), not drift.

A beads repo without a podras brief gets no render gate; export drift there is
covered by the existing export-drift guard.

## Canonical beads semantics

`ddl init` writes an AGENTS.md beads block whose storage-semantics statement is:

- the live database is per-machine,
- `.beads/issues.jsonl` is the committed export (output of `bd export`), and
- git is the sync mechanism.

`ddl doctor` flags any repo whose beads block asserts the export file itself is the
source of truth, naming the file and the incorrect claim, and suggests re-running
`ddl init --gates` to rewrite the managed block.

## See also

- [Implementation Status](./status.md)
- [`health` spec](./specs/health.md)
- [`dot-ddl` spec](./specs/dot-ddl.md)
