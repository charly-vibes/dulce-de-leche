# Fleet session retrospective — 60 days of pi / omp / codex

**Date:** 2026-11-25 · **Window:** 2026-09-25 → 2026-11-25 (60 days)
**Question:** what improvements does the dulce-de-leche ecosystem need, judged from how the
fleet actually works (repeated failures, good behaviours, missed gates)?

## Method

- **Sources:** `~/.pi/agent/sessions` (2,117 sessions), `~/.omp/agent/sessions` (1,569),
  `~/.codex/sessions` (126) = **3,812 sessions**, ~4.8 GB of logs, 49 repos.
- **Extraction:** streamed every session log; per session captured cwd, model, tool calls,
  every `bash` command string, tool errors (isError / traceback / non-zero exit), and
  gate/behaviour signals (regex on executed commands, not on prose).
- **Instruction-presence probe:** for each session, searched the *prompt* for the fleet
  protocol markers (global AGENTS.md, beads tracked-export invariant, `wai close`,
  MANDATORY push workflow, `just ci`, `tdd-ro5`).
- **Live fleet state:** `git status`/`@{u}..HEAD` per repo; `bd export` diffed against the
  tracked `.beads/issues.jsonl`; `ddl doctor --json`.
- **Caveat:** session-level attribution. A commit may happen in a later session
  (75 % of omp no-commit sessions had a commit session in the same repo within 24 h —
  see F1). Numbers are "sessions where the behaviour was observed", not "work items".

---

## Findings

### F1 — Harness asymmetry is the root cause of missed gates (highest impact)

The fleet protocol lives in the **global** `~/AGENTS.md`. Only pi traverses up to it.

| marker in session prompt | pi | omp | codex |
|---|---|---|---|
| repo `AGENTS.md` (beads block) | 83 % | 57 % | 91 % |
| global AGENTS.md ("Global Agent Instructions") | 94 % | 0 % | 2 % |
| "Work is NOT complete until `git push` succeeds" | 66 % | **0 %** | 28 % |
| beads tracked-export invariant | 87 % | **1 %** | 4 % |
| wai managed block / `wai close` ritual | 94 % | 3 % | — |

Consequences, same repos, same models (glm-5.3-flash is the top model in both harnesses):

| harness | edit-sessions | ran a test/gate | committed | pushed | used `bd` |
|---|---|---|---|---|---|
| pi | 1,257 | 69 % | **89 %** | 61 % | 84 % |
| omp | 1,166 | 38 % | **13 %** | 10 % | 24 % |

Not a model-capability gap — an **instruction-materialization** gap. 270 omp edit-sessions
had no follow-up commit in the same repo within 24 h (work left in the tree).

### F2 — The beads tracked-export invariant is violated right now

- 5/17 fleet repos have a **stale** tracked `.beads/issues.jsonl` vs live `bd export`:
  `wai` (2 issues), `pretender` (7), `specodelic` (2), `kaya` (1), `sunset` (5) — including
  closed issues missing from the tracked file and new issues missing from it.
- 181 sessions (204 occurrences) hit `The following paths are ignored by one of your
  .gitignore files: .beads` — the global `~/.gitignore` swallows `.beads/`.
- The workaround is used in **631 sessions (16.6 %)**: `git add -f .beads/issues.jsonl`.
  It is not discoverable — every session relearns it, and some forget.

### F3 — The session-lifecycle ritual is mostly not executed

`wai close` 6.6 % of sessions · `wai next` 0.1 % · `wai review` 0.1 %.
Yet the rituals correlate strongly with finishing the work (edit-sessions):

| behaviour | n | ran a gate | committed | pushed |
|---|---|---|---|---|
| ran `bd export` | 772 | 78 % | **95 %** | 67 % |
| ran no `bd export` | 1,651 | 43 % | 33 % | 22 % |
| used the review/RO5 skill | 811 | 67 % | 78 % | 55 % |
| did not | 1,612 | 48 % | 40 % | 27 % |
| used `wai pipeline` | 341 | 70 % | 83 % | 62 % |
| did not | 2,082 | 52 % | 47 % | 33 % |

The ritual is the strongest predictor of "work actually got closed".

### F4 — 37 % of edit-sessions never run any gate

Per repo (main sessions only): best `charly/bajan` 98 %, `charly/wai` 96 %,
`charly/gently` 96 %, `charly/wild` 98 %, `charly/testaruda` 100 % — the repos that live by
`just ci` + lefthook. Worst: `charly/sunset` 2 % (omp) / 19 % (pi), `at/bogart` 4 %,
`charly/sad` 36 %, `charly/kaya` 42 %, `charly/podras` 76 % but 7 % commit.
`just ci` itself appears in only 11.8 % of sessions; `cargo test` 25 %.

### F5 — Harness-level friction wastes turns

- pi `edit` exact-match failures: **1,438 over 17,531 edit calls (8.2 %)**, touching
  32 % of pi sessions ("Could not find the exact text", "Found 2 occurrences",
  "Edits … resulted in no changes").
- Tool-schema rejections: "Validation failed for tool edit/read/bash", "Tool call arguments
  are not valid JSON" — dozens per session-class.
- **330 bash timeouts** (120 s / 300 s / 600 s / 900 s) — long `cargo`/CI gates killed
  mid-flight, then re-run from scratch.

### F6 — Skill path drift (122 sessions)

Agents read `~/.pi/agent/skills/<name>/SKILL.md`, which does not exist; the canonical root is
`~/.agents/skills/`. Failures: `renew` 44, `rule-of-5-universal` 34 (+7 for its references),
`next` 9, `close` 4. They guess a uniform root instead of resolving the real one.

### F7 — Adoption of the 17 managed tools is very uneven

Session-level CLI usage: `git` 70 %, `openspec` 52 %, `bd` 46 %, `just` 35 %, `wai` 32 %,
`cargo` 32 %, `pretender` 26 %, `ah` 25 %, `spk` 23 %, `testaruda` 9 %, `dont` 5 %,
`vampiro` 4 %, `ddl status/doctor` **1.3 %**, `funkalization` 1 %.
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

### F9 — Fleet state debt today

- 48/50 repos have a dirty working tree (`charly/kaya` 28 files, `charly/wai` 23,
  `charly/sunset` 12 **+ 4 unpushed commits**, `charly/sunshine` **26 unpushed commits**).
- `ddl doctor` in the ddl repo: `fail` — `bajan not installed`; warns — pre-commit and
  pre-push managed blocks missing, `AGENTS.md` missing the ddl managed block,
  `.testaruda/` not gitignored; 16 tools "not tracked in manifest".
- Known gap already documented in `docs/standardization.md §9.2`: the generated gate blocks
  do not chain `bd hooks run` → `ddl.repo.bd-hooks` can never pass on a `ddl init`-wired repo.

### F10 — Good behaviours worth amplifying

- pi discipline: 89 % of edit-sessions commit; `wai pipeline tdd-ro5` used in 341 sessions.
- `bd export` sessions commit at 95 % — the ritual works when it is in context.
- `charly/bajan`, `wai`, `testaruda`, `gently`, `wild`, `espectacular`, `wanna`, `genesis`
  show 95–100 % gate coverage and 90 %+ commit — the "just ci + lefthook + doctor" pattern works.
- `canticos/terapia` already parses pi sessions for post-mortems; the same job is done here
  by hand for three harnesses — it should be a fleet feature, not a one-off.

---

## Proposed improvements (candidate backlog for the ecosystem)

### P0 — Materialize the protocol where every harness looks

1. **Session-protocol managed block.** `ddl init --gates` writes a
   `SESSION-PROTOCOL` block into every fleet repo's `AGENTS.md` (the machinery already
   exists: `src/installer/init.rs::agents_block`, `ddl.repo.blocks` doctor check). Content:
   RED→GREEN order, `just ci` as the only gate, `bd export -o .beads/issues.jsonl &&
   git add -f .beads/issues.jsonl && git commit`, `git push`, `wai close`. Kills F1, F3, F4.
2. **Beads export guard.** New doctor check `ddl.repo.beads-export` comparing live
   `bd export` with the tracked file (today 5 repos fail); fix the §9.2 chain gap so
   `bd hooks run pre-commit/pre-push` is emitted by the generated blocks; add a repo-local
   `.gitignore` negation for `.beads/` so plain `git add` stops needing `-f`. Kills F2.

### P1 — Make the gates cheap and observable

3. **`ddl fleet report --last 60d`** — parse pi/omp/codex logs (extend `canticos/terapia`
   to omp + codex) and report gate coverage, error hotspots, uncommitted work, tool
   adoption. Turns this retrospective into a fleet feature and gives `ddl status` teeth.
4. **`ddl gates run`** — long gates in background with progress + tail, immune to the 330
   timeouts; and a doctor check `ddl.repo.just-ci` for repos without a `just ci` recipe.
5. **`ddl doctor --adoption`** — per repo: which managed tools were invoked in the last N
   days. Makes the 17-tool registry self-justifying (F7).

### P2 — Remove the sharp edges

6. **Skill-root resolution.** `ddl init` links/points the canonical skill root
   (`~/.agents/skills`) into every harness's skill directory, or the fleet AGENTS block
   states the single canonical root. Kills F6.
7. **CLI ergonomics.** `wai`: "did you mean" for unknown flags; infer the project from cwd
   instead of erroring with "Multiple projects found"; fix `wai feedback bug --from-last-error`
   (it must not treat an error string as an artifact/filename); friendlier `wai todo` errors.
   Kills F8.
8. **Edit-tool ergonomics.** Exact-match-only `edit` costs 8.2 % of edit calls; either
   pi gains a fuzzy/anchored variant, or the fleet ships a "read-then-edit" convention
   (or an `edit` preflight that validates uniqueness before writing). Kills F5 partially.

## Sequencing suggestion

P0-1 and P0-2 are small, mechanical, testable changes in `dulce-de-leche` itself with
immediate fleet-wide effect; they are the ones with the best effort/benefit ratio.
P1-3 is the enabler for measuring whether P0 worked.
Everything else can follow once the protocol is uniformly visible and the fleet can be
measured without hand-rolled scripts.
