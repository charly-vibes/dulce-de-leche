# Status SSOT friction — pi/omp session mining + repo landscape audit

**Date:** 2026-11-27 · **Question:** where does the Single Source of Truth for a repo's
status live, and what friction do agents hit reconciling it? Feeds the next round of
dulce-de-leche improvements.
**Method:** (a) qualitative mining of pi + omp session logs across charly repos
(~3,700 sessions, ~5.3 GB) for real friction episodes — not echoed instruction text;
(b) repo-side audit of which status artifacts exist, who produces them, and how stale
each is; (c) error-class mining of status-command failures.
**Status today:** `podras brief --json` is the *declared* machine-readable status
contract (`BRIEF.json`/`BRIEF.md` committed render), bd (`.beads/issues.jsonl`) the
declared issue SSOT, `AGENT-SUMMARY.md` a hand-written session summary at repo root,
`wai status` / `bd list` / `ddl status` / `openspec` auxiliary views. `ddl` manages
*tool* status but has no notion of *project* status.

---

## The SSOT landscape (measured 2026-11-27)

Last-touch dates (git) per repo:

| repo | BRIEF.md render | AGENT-SUMMARY.md | issues.jsonl | brief-drift gate armed |
|---|---|---|---|---|
| dulce-de-leche | 11-27 ✅ | 10-20 (stale, tracked) | 11-27 | ✅ |
| podras | 11-19 | — | 11-25 | ✅ |
| wai | **11-18** | — | 11-25 | ✗ |
| dont | **11-18** | — | 11-25 | ✗ |
| espectacular | **11-18** | — | 11-25 | ✗ |
| pretender | **11-18** | — | 11-27 | ✗ |
| specodelic | **11-18** | — | 11-27 | ✗ |
| vampiro | **11-18** | — | 11-27 | ✗ |
| whisper | **11-18** | — | 11-18 | ✗ |
| incitaciones | **11-18** | — | 11-25 | ✗ |

**8 of 10 repos have committed brief renders 7–9 days staler than their own issue
tracker, and only 2 of 10 arm the gate that would catch it.** The mechanism that
makes the render trustworthy exists (podras `--write` + `brief-drift` hook) but was
never rolled out beyond the two repos that built it. The AGENTS.md instruction
("run `podras brief --json` first") points agents at a *live* render that often
disagrees with the *committed* one they are reading.

## Friction classes (session evidence)

Ranked by frequency × cost. All quotes verbatim from session logs.

### F-A. Committed render vs live source — the dominant pattern
Every status artifact that is *committed* (issues.jsonl, BRIEF.md/BRIEF.json,
AGENT-SUMMARY.md, status pages) drifts from the *live* store (bd DB, podras brief,
actual work), and agents pay reconciliation tax on every orientation.

- bd export drift: 10 of 44 beads repos had stale tracked exports (60d retro,
  re-verified); bd `close`/`sync` does not update the JSONL ("bd sync does NOT
  reconcile .beads/issues.jsonl — the jsonl stayed stale ('open') after sync reported
  complete", charly fleet 2026-10-09, `01a121ea…jsonl`). bd auto-export fails
  silently ("Warning: auto-export: git add failed: exit status 1", dont 2026-05-05,
  wai 2026-05-12).
- Brief renders: `brief-drift: BRIEF.md drifted from a fresh render` blocks commits
  (genesis 2026-11-25/27, dulce 2026-11-25 ×3) — and where the gate is *absent*
  (8 repos) nothing flags the drift at all: sunset's agent kept a private JSONL plan
  store that "said both OPEN … live bd showed CLOSED" (2026-11-24); kaya's
  `docs/src/status.md:21` said "no release tagged" while v0.1.0 was published
  (2026-11-18); tambor's README/project.md stale status flagged CRITICAL by its own
  Rule-of-5 review (2026-11-16).
- AGENT-SUMMARY.md: only dulce has it; it is hand-written, tracked at root,
  provably stale (from DDL-fw7, 5+ weeks), caused merge conflicts
  ("junk-file churn from the agents committing summaries", dulce 2026-10-14) and a
  forensic detour in a fresh worktree ("How is it in this brand-new worktree?",
  2026-10-20). Even an omp orchestrator didn't know its provenance
  ("AGENT-SUMMARY.md? root — check if generated", 2026-11-25).

### F-B. bd storage semantics have no consistent fleet definition
The single most frequent reconciliation trigger (~7 verified episodes):
- AGENTS.md blocks contradict *across* repos (canticos/genesis: "jsonl … source of
  truth" vs testaruda's own hook output: "issues.jsonl is an export, not cross-machine
  sync or source of truth") and *within* one repo (canticos 2026-11-13: "AGENTS.md
  says … jsonl source of truth. But the 14:50 handoff said no-db:false, dolt source
  of truth. Conflicting.").
- Agents re-derive the truth model from scratch per repo: genesis 2026-11-27
  (".beads/issues.jsonl … contains 0 matches for genesis-qmc — the bd state changes
  are NOT in the jsonl!"), coffee 2026-11-11 ("bd said '✓ Closed' … git status shows
  no change"), testaruda 2026-11-27 ("my amend merged MY fix into a beads export
  commit??").
- The Dolt→no-db migration was executed under a wrong SSOT belief and corrected ~10
  days later (dulce 2026-09-28 → 2026-10-08); stale `bd dolt push` mandates lingered
  in AGENTS.md blocks for months.

### F-C. Status commands fail or lie at orientation time
- `wai` multi-project error is the most-repeated verbatim status-command failure
  ("Multiple projects found … Use `--project`"): 75+ files corpus-wide, 27 files in
  sunset alone on one day (2026-11-24). Already ticketed (DDL-9kz.9.2) but the
  counts show it is worse than the 11 sessions the ticket cites.
- `wai status` fails outright in repos without `.wai/` (coffee 2026-11-26
  `wai::project::config_missing`; whisper 2026-09-30 hung), exits differently across
  versions (bajan CI red / local green, 2026-11-11), and emits non-corresponding
  error IDs in `--json` vs human mode (wai-4hqy).
- `ddl status`/`doctor` output shape drifts across versions (0.3 `data.checks` vs
  0.5 `data.diagnostics`), prints nulls ("ddl: error — null"), classifies every pass
  as a warning (`pass: 0, warn: 29, fail: 0`, REPLy.jl #46), and — as a side effect —
  *mutates the repo* while probing tools (`testaruda creates .testaruda/ before
  failing`, dont CI DDL-2zu).
- `podras brief` false-positived on 23 "malformed lines" that were all valid JSON
  (dulce 2026-11-21, filed DDL-kk9) — the status contract was untrustworthy at the
  moment it was most needed.

### F-D. No definition of *where repo status lives* → invented + hand-edited stores
When no artifact is trusted, agents build their own: sunset's PlanScout private JSONL
plan store; specodelic subagent hand-editing the epic run YAML ("never hand-edit run
YAMLs" now repeated in every handoff); kaya/sunset agents manually patching bd
derived status (`bd ready` excluded a de-facto-unblocked issue). Stale handoffs are
treated as hypotheses to re-verify against bd every resume (wai 2026-11-21: "The
handoff itself said verify/may be stale — confirmed: stale"). Cross-repo resume
routing is undefined ("/renew was called from the charly/dont repo … but the
in-flight is genesis-side. Should /renew target the DONT repo then?").

## Root causes

1. **Every status artifact is a cache, but nothing declares it one.** issues.jsonl,
   BRIEF renders, AGENT-SUMMARY.md, status pages are all derived; AGENTS.md blocks
   call some of them "source of truth" while tool output calls the same files "an
   export".
2. **The drift-detection mechanism (brief-drift gate, bd export guard) exists but is
   armed in 2/10 repos** — `ddl init --gates` covers repos it has been re-run on
   (DDL-44k is already open for this).
3. **ddl has no project-status concept.** `ddl status`/`doctor` manage tool health;
   nothing in the 17-tool ecosystem owns "what is this repo's status, where is it,
   is it fresh".
4. **Status producers don't share a freshness contract.** podras brief has
   `staleness` fields; bd, wai status, AGENT-SUMMARY.md, status pages don't.

## Improvement directions (candidates for a ddl proposal)

1. **Roll out + make mandatory the render-freshness loop** — extend the
   `brief-drift` pre-commit gate (exists in dulce/podras lefthooks) into a
   ddl-managed gate written by `ddl init --gates` for every repo with a
   `.beads/issues.jsonl`; DDL-44k (`re-run ddl init --gates` fleet-wide) is the
   vehicle. Gate order must be documented: bd write → `podras brief --write` →
   `bd export` → commit (the ordering confusion cost 3 commit retries).
2. **One canonical bd-semantics block** — ddl generates the AGENTS.md beads block
   with the *correct* statement ("issues.jsonl is the committed export, git is the
   sync; the live DB is per-machine"), and a `ddl doctor` check flags any repo whose
   beads block claims jsonl is the SSOT (currently contradicted by hook output in
   testaruda and by handoff notes in canticos).
3. **`ddl status` grows a project-status section** (or `ddl repo status`): reads
   `podras brief --json` (already a typed envelope with staleness), bd
   open/blocked counts, and renders one answer — "repo status = brief.as_of +
   tracker counts + staleness flags", replacing the orientation dance of 4 commands.
   A doctor check `ddl.repo.brief-fresh` fails when the committed render is older
   than the tracker's last change (the 8-repo gap above is exactly this).
4. **Give AGENT-SUMMARY.md a home or delete it** — session summaries belong under
   `.wai/` (already the proposal in dulce's 2026-10-14 audit, never executed) or
   should be generated/managed like the brief; a tracked root file with no producer
   will keep causing merge churn and staleness in every repo that copies the
   convention.
5. **Status artifacts declare their producer + freshness in-band** — BRIEF renders
   already carry `generated_at`; the same footer convention for AGENT-SUMMARY.md,
   handoffs, and status pages ("generated by X at T; verify against Y") converts
   "is this current?" from a forensic investigation into a glance.
6. **Fix the observer tools first** — podras malformed-line false positive
   (DDL-kk9), wai multi-project resolution from cwd (DDL-9kz.9.2 — evidence shows
   it is much bigger than ticketed), `ddl doctor` warn-noise and repo-mutating
   probe (DDL-2zu). A status SSOT built on lying observers just moves the
   reconciliation cost.

## Reproduce

```bash
# render staleness vs tracker, per repo
for r in dulce-de-leche podras wai dont espectacular pretender specodelic vampiro whisper incitaciones; do
  (cd $r && echo "$r brief=$(git log -1 --format=%cs -- BRIEF.md) jsonl=$(git log -1 --format=%cs -- .beads/issues.jsonl)")
done
# gate presence
rg -l 'brief-drift' ../{podras,dulce-de-leche}/lefthook.yml
# bd export drift (10 stale repos, per 60d retro method)
for d in ../*/; do (cd "$d" && bd export -o /tmp/e.jsonl && diff -q /tmp/e.jsonl "$d/.beads/issues.jsonl" >/dev/null || echo "$d STALE"); done
# session evidence: rg the quote strings above in ~/.pi/agent/sessions and ~/.omp/agent/sessions
```
