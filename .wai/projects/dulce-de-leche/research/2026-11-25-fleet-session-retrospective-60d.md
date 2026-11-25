# Fleet session retrospective — 60 days of pi / omp / codex

**Date:** 2026-11-25 · **Window:** 2026-09-25 → 2026-11-25 (60 days)
**Question:** what improvements does the dulce-de-leche ecosystem need, judged from how the
fleet actually works (repeated failures, good behaviours, missed gates)?
**Backlog filed as:** `DDL-9kz` (epic) + 11 children — see
[`.beads/issues.jsonl`](../../../../.beads/issues.jsonl) or `bd show DDL-9kz`.

> **Corrections applied 2026-11-25** after a Rule-of-5 review of this artifact (tracked as
> `DDL-9kz.11`): the beads-violation count, the instruction-presence table, the dirty-repo
> counts, the timeout count, the repo-scope sentence and the codex outcome row were re-measured
> and corrected. Marked with *(corrected)* where they changed.

## Method

- **Sources:** `~/.pi/agent/sessions` (2,118 sessions), `~/.omp/agent/sessions` (1,570),
  `~/.codex/sessions` (126) = **3,812 sessions**, ~4.8 GB of logs, **121 distinct working
  directories** *(corrected: was "49 repos")*. Of those, 54 are git repos under
  `gh/charly/` (49) and `gh/at/` (5); the rest are `ak/*`, `tds/*` and `/tmp` sessions.
- **Extraction:** streamed every session log; per session captured cwd, model, tool calls,
  every `bash` command string, tool errors (isError / traceback / non-zero exit), and
  gate/behaviour signals (regex on executed commands, not on prose).
- **Instruction-presence probe:** for each session, searched the *whole log* for the fleet
  protocol markers (global AGENTS.md, `MANDATORY WORKFLOW`, beads tracked-export invariant,
  `wai close`, `just ci`, `tdd-ro5`).
- **Live fleet state:** `git status`/`@{u}..HEAD` per repo; `bd export` diffed against the
  tracked `.beads/issues.jsonl`; `ddl doctor --json`.
- **Caveats:** (a) session-level attribution — a commit may happen in a later session
  (74 % of omp no-commit sessions had a commit session in the same repo within 24 h —
  see F1); (b) subagent exclusion is heuristic (pi by `session_info.name`: 447 `subagent:*`
  but 118 other-named sessions are also subagents; omp by filename role pattern) → pi's
  rates are slightly pessimistic; (c) small-n cells are directional only; (d) **not measured**:
  code quality of the resulting diffs, test coverage, cost, and codex outcome metrics.

### Reproduce

```bash
# session corpus + per-session telemetry (ship as `ddl fleet report` — DDL-9kz.5)
python3 /var/home/sasha/tmp/sess_analysis/extract.py && python3 /var/home/sasha/tmp/sess_analysis/pass2.py
# beads export drift, per repo
for d in ../*/ ; do (cd "$d" && bd export -o /tmp/e.jsonl) && diff /tmp/e.jsonl "$d/.beads/issues.jsonl"; done
# dirty trees / unpushed commits
for r in ../*/ ; do echo "$r $(git -C "$r" status --porcelain | wc -l) $(git -C "$r" log --oneline @{u}..HEAD | wc -l)"; done
# ddl doctor
ddl doctor --json
```

---

## Findings

### F1 — Harness asymmetry is the root cause of missed gates (highest impact)

The fleet protocol lives in the **global** `~/AGENTS.md`. Only pi traverses up to it.

*(corrected — full-log scan of every in-window session, not just the prompt head)*

| marker present in session | pi (n=2118) | omp (n=1570) | codex (n=126) |
|---|---|---|---|
| repo `AGENTS.md` (beads block) | 83 % | 59 % | 92 % |
| global AGENTS.md ("Global Agent Instructions") | 94 % | 0 % | 4 % |
| push protocol (`MANDATORY WORKFLOW`) | 66 % | **1 %** | 29 % |
| beads tracked-export invariant (`no-db: true`) | 88 % | **2 %** | 13 % |
| `wai close` mentioned | 95 % | 11 % | 88 % |
| `just ci` mentioned | 34 % | 29 % | 17 % |
| `tdd-ro5` mentioned | 33 % | 31 % | 53 % |

**The missing piece is specifically the session-close protocol, not the discipline protocol:**
omp *does* receive repo-level rules (`just ci` 29 %, `tdd-ro5` 31 %, repo beads block 59 %);
it never receives the close ritual (commit → `bd export` → push → `wai close`).

Consequences, same repos, same models (glm-5.3-flash is the top model in both harnesses),
main edit-bearing sessions:

| harness | edit-sessions | ran a test/gate | committed | pushed | used `bd` |
|---|---|---|---|---|---|
| pi | 1,257 | 69 % | **89 %** | 61 % | 84 % |
| omp | 1,166 | 38 % | **13 %** | 10 % | 24 % |
| codex | — | — | — | — | — |

*(corrected: codex outcome metrics were not extracted — the extractor did not recognise
codex's tool names, so the zero row was a measurement artifact, not a finding.)*

Not a model-capability gap — an **instruction-materialization** gap. 270 omp edit-sessions
had no follow-up commit in the same repo within 24 h (74 % of the 1,018 omp no-commit
sessions *did* — work left in the tree).

### F2 — The beads tracked-export invariant is broken right now

*(corrected — re-measured across all 44 repos that have beads, both orgs)*

**10 of 44** repos have a **stale** tracked `.beads/issues.jsonl` vs live `bd export`:

| org | repo | diff lines |
|---|---|---|
| `gh/charly` | `pretender` | 7 |
| `gh/charly` | `canticos` | 5 |
| `gh/charly` | `sunset` | 5 |
| `gh/charly` | `kaya` | 3 |
| `gh/charly` | `espectacular` | 2 |
| `gh/charly` | `specodelic` | 2 |
| `gh/charly` | `tambor` | 2 |
| `gh/charly` | `wai` | 2 |
| `gh/at` | `bogart` | 20 |
| `gh/at` | `catherine` | 4 |

Both directions occur: closed issues missing from the tracked file (`wai-a6uy`, `wai-wicn`)
and newly created issues missing from it (`sunset` ×5, `kaya-eaj`).
`homebrew-charly`, `scoop-charly` and `at/arltificial` have no `.beads/` at all — *not
applicable*, not violations.

- 181 sessions (204 occurrences) hit `The following paths are ignored by one of your
  .gitignore files: .beads` — the global `~/.gitignore` swallows `.beads/`.
- The workaround appears in **2,338 `git add -f` command lines** (631 sessions, 16.6 %),
  the large majority force-adding `.beads/issues.jsonl` — often as part of the ritual line
  agents copy verbatim. The fix must *change that line*, not add a new one.

### F3 — The session-lifecycle ritual is mostly not executed

`wai close` 6.6 % of sessions · `wai next` 0.1 % · `wai review` 0.1 %.
Yet the rituals correlate strongly with finishing the work
(**denominator: main edit-bearing sessions**):

| behaviour | n | ran a gate | committed | pushed |
|---|---|---|---|---|
| ran `bd export` | 772 | 78 % | **95 %** | 67 % |
| ran no `bd export` | 1,651 | 43 % | 33 % | 22 % |
| used the review/RO5 skill | 811 | 67 % | 78 % | 55 % |
| did not | 1,612 | 48 % | 40 % | 27 % |
| used `wai pipeline` | 341 | 70 % | 83 % | 62 % |
| did not | 2,082 | 52 % | 47 % | 33 % |

*(corrected: over **all** sessions the same signals appear 858 × `bd export`,
1,129 × review/RO5, 451 × `wai pipeline` — the table above is edit-bearing sessions only.)*

The ritual is the strongest predictor of "work actually got closed".

### F4 — 37 % of edit-sessions never run any gate

Per repo (main sessions only): best `charly/bajan` 98 %, `charly/wild` 98 %, `charly/testaruda`
100 %, `charly/gently` 96 %, `charly/wai` 96 % — the repos that live by `just ci` + lefthook.
Worst: `charly/sunset` 2 % (omp) / 19 % (pi), `at/bogart` 4 %, `charly/sad` 36 %,
`charly/kaya` 42 %, `charly/podras` 76 % but 7 % commit. `just ci` itself appears in only
11.8 % of all sessions; `cargo test` 25 %.

### F5 — Harness-level friction wastes turns

- pi `edit` exact-match failures: **1,438 over 17,531 pi edit calls (8.2 %)**, touching
  32 % of pi sessions ("Could not find the exact text", "Found 2 occurrences",
  "Edits … resulted in no changes"); omp 459 / 17,562 (2.6 %).
- Tool-schema rejections: **192** ("Validation failed for tool edit/read/bash",
  "Tool call arguments are not valid JSON").
- **306** bash timeouts *(corrected: was "330")* — `Command timed out after 120/300/600/900 s`
  kills long gates mid-flight, then they are re-run from scratch.

### F6 — Skill path drift (~120 occurrences)

Agents read `~/.pi/agent/skills/<name>/SKILL.md`, which does not exist; the canonical root is
`~/.agents/skills/`. Failures: `renew` 44, `rule-of-5-universal` 34 (+7 for its references),
`next` 9, `close` 4. They guess a uniform root instead of resolving the real one.

### F7 — Adoption of the 17 managed tools is very uneven

Session-level CLI usage (all sessions): `git` 70 %, `openspec` 52 %, `bd` 46 %, `just` 35 %,
`wai` 32 %, `cargo` 32 %, `pretender` 26 %, `ah` 25 %, `spk` 23 %, `testaruda` 9 %,
`dont` 5 %, `vampiro` 4 %, `ddl status/doctor` **1.3 %**, `funkalization` 1 %.
`ddl` manages 17 tools but never checks whether they are used — and `ddl doctor` itself is
almost never run.

### F8 — CLI discoverability errors (agents guess flags)

- Unknown flags: `--note`, `--labels`, `--text`, `--notes`, `--add-comment`, `--add-labels`,
  `--cached` — 86 occurrences / 78 sessions.
- `wai` in a multi-project workspace: "Multiple projects found … use `--project`" — 11 sessions.
- **`wai feedback bug --from-last-error` is effectively broken**: it resolves the last error
  as an artifact → `artifact '1' not found in project 'finanzas'`,
  `review target must be a filename, not a path: '.wai/projects/…/reviews/….md'`
  (26 occurrences, 17 sessions).
- `wai todo done` misuse: "Errors: done needs a task, items or a phase" (11 sessions).

### F9 — Fleet state debt today *(snapshot 2026-11-25 ~13:00–13:40 local; volatile)*

- **49 of 54** git repos have a dirty working tree *(corrected: was "48/50")* — `charly/wai`
  24 files, `charly/sunset` 12 **+ 4 unpushed commits**, `charly/sunshine` **26 unpushed
  commits**. Numbers move: `charly/kaya` measured 28 dirty files and then 0 within the same
  working session — re-run the commands in *Reproduce* before citing a per-repo figure.
- `ddl doctor` in the ddl repo: `fail` — `bajan not installed`; warns — pre-commit and
  pre-push managed blocks missing, `AGENTS.md` missing the ddl managed block,
  `.testaruda/` not gitignored, and **14** tools "not tracked in manifest" *(corrected: 16)*.
  (Already filed as `DDL-v4s`.)
- Known gap already documented in `docs/standardization.md §9.2`: the generated gate blocks
  do not chain `bd hooks run` → `ddl.repo.bd-hooks` can never pass on a `ddl init`-wired repo
  (filed as `DDL-9kz.4`).

### F10 — Good behaviours worth amplifying

- pi discipline: 89 % of edit-sessions commit; `wai pipeline tdd-ro5` used in 341 edit-sessions
  (451 all sessions).
- `bd export` sessions commit at 95 % — the ritual works when it is in context.
- `charly/bajan`, `wai`, `testaruda`, `gently`, `wild`, `espectacular`, `wanna`, `genesis`
  show 95–100 % gate coverage and 90 %+ commit — the "just ci + lefthook + doctor" pattern works.
- `canticos/terapia` already parses pi sessions for post-mortems; the same job was done here
  by hand for three harnesses — it should be a fleet feature, not a one-off (`DDL-9kz.5`).

---

## Proposed improvements (backlog = `DDL-9kz` and children)

### P0 — Materialize the protocol where every harness looks

1. **Session-protocol managed block** — `DDL-9kz.1`. `ddl init --gates` writes a
   `SESSION-PROTOCOL` block into every fleet repo's `AGENTS.md` (machinery already exists:
   `src/installer/init.rs::agents_block`, `ddl.repo.blocks` doctor check). Content:
   RED→GREEN order, `just ci` as the only gate, `bd export -o .beads/issues.jsonl &&
   git add -f .beads/issues.jsonl && git commit`, `git push`, `wai close`.
   *Expected effect: omp edit-session commit rate 13 % → ≥ 30 %.*
2. **Beads export guard** — `DDL-9kz.2` (doctor check `ddl.repo.beads-export`),
   `DDL-9kz.3` (remediate the 10 stale repos), `DDL-9kz.4` (emit the `bd hooks run` chain).
   Also: a repo-local `.gitignore` negation for `.beads/` so plain `git add` stops needing `-f`.

### P1 — Make the gates cheap and observable

3. **`ddl fleet report --last 60d`** — `DDL-9kz.5`. Parse pi/omp/codex logs (extend
   `canticos/terapia`, do not ship a second parser) and report gate coverage, error hotspots,
   uncommitted work, adoption.
4. **`ddl gates run` + `ddl.repo.just-ci`** — `DDL-9kz.6`. Long gates in background with
   progress + tail, immune to the 306 timeouts.
5. **`ddl doctor --adoption`** — `DDL-9kz.7`. Per repo: which managed tools were invoked.

### P2 — Remove the sharp edges

6. **Skill-root resolution** — `DDL-9kz.8`.
7. **CLI ergonomics (wai)** — `DDL-9kz.9`: "did you mean" for unknown flags; infer the project
   from cwd; fix `wai feedback bug --from-last-error`; friendlier `wai todo` errors.
8. **Edit-tool ergonomics** — `DDL-9kz.10`: exact-match-only `edit` costs 8.2 % of pi edit
   calls; either pi gains an anchored variant, or the fleet ships a "read-then-edit" convention.

### Docs

9. **Fleet incident + standardization §10 + corrections to this artifact** — `DDL-9kz.11`.

## Sequencing suggestion

P0-1 and P0-2 are small, mechanical, testable changes in `dulce-de-leche` itself with
immediate fleet-wide effect; they are the ones with the best effort/benefit ratio.
P1-3 is the enabler for measuring whether P0 worked.
Everything else can follow once the protocol is uniformly visible and the fleet can be
measured without hand-rolled scripts.
