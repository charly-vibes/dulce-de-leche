# Tasks

## 1. Status record core (repo-status capability)

- [ ] 1.1 Create `src/repo_status.rs`: `repo-status.v1` record types (versioned, tolerant envelope), serialization, and the read path used by `ddl status`
- [ ] 1.2 `FeedObtainer` trait (subprocess protocol, per-tool timeout, error capture) + descriptor field `status_feed` on the tool table in `src/platform/tool.rs` (podras, bd, openspec, git)
- [ ] 1.3 Feed obtainers: podras `brief --json` envelope, bd `bd list --json` counts, git branch/ahead-behind/dirty via genesis git helpers, openspec `openspec list --json`
- [ ] 1.4 Staleness engine per design D2: brief feed = committed `BRIEF.json` vs fresh envelope diff; beads feed = tracked `.beads/issues.jsonl` vs `bd export` temp diff; git = live. Unit tests pin the whisper-class case (tracker unchanged ⇒ not stale) using content diff, not timestamps
- [ ] 1.5 `ddl status --refresh`: dot-ddl file lock, write ordering (obtain → `podras brief --write` when applicable → `bd export` → record write), per-step reporting, actionable failure output naming the remaining command
- [ ] 1.6 Contract-precedence in-band header: emitted outputs identify ddl + `generated_at` + live record as source of truth (spec scenario)
- [ ] 1.7 Unmanaged status artifact detection: root-level hand-written status files (`AGENT-SUMMARY.md` class) reported with last-commit date

## 2. `ddl status` + `ddl doctor` integration (health capability)

- [ ] 2.1 `ddl status`: project-status section in human output + `project` object in `--json` (in-flight summary, tracker counts, per-feed `observed_at`/`staleness`); no-record path renders suggestion without affecting exit code
- [ ] 2.2 `ddl doctor`: `ddl.repo.*` checks under a separate `repo` JSON key and report section — `status-fresh`, `beads-block`, `brief-drift-gate`, `unmanaged-status-artifacts`
- [ ] 2.3 `ddl doctor --fix`: stale/missing record refreshed via the ordering; reported as an applied fix
- [ ] 2.4 Error-identity parity: repo-check failures carry the same error codes in `--json` and human output

## 3. Gate emission + AGENTS.md blocks (repo-status capability)

- [ ] 3.1 Brief-drift gate template emitted by `ddl init --gates` for repos with a committed brief render; failure message states the required ordering (shared constant with 1.5)
- [ ] 3.2 No-render beads repos: skip render gate, keep export-drift guard; covered by a doctor check, not a gate
- [ ] 3.3 Canonical AGENTS.md beads-semantics block written by `ddl init` (live DB per-machine, jsonl = committed export, git = sync); `beads-block` doctor check flags blocks claiming the export is the source of truth
- [ ] 3.4 Migrate the hand-rolled brief-drift gates in dulce-de-leche and podras lefthook.yml to the ddl template (delete the forks)

## 4. Tests + verification

- [ ] 4.1 Integration tests: refresh idempotent on re-run; gate passes after the ordering without manual retry (the 3-retry regression); feed-unavailable degradation (podras absent repo still exits 0 with `unavailable` feeds)
- [ ] 4.2 Doctor tests: contradictory beads block flagged; correct block passes; repo failures don't alter tool counts
- [ ] 4.3 Run `just test` (testaruda loop) for the working diff; full gates via lefthook pre-push

## 5. Fleet rollout + ticket reconciliation

- [ ] 5.1 Prerequisite gate: confirm DDL-kk9 (podras malformed-line FP) closed before rollout
- [ ] 5.2 Re-scope DDL-852c and DDL-4jvz to this change (note in beads); DDL-js72 satisfied by 3.3, DDL-qfc5 decision input from 1.7 evidence
- [ ] 5.3 Fleet rollout via DDL-44k: re-run `ddl init --gates` across charly repos; `ddl doctor` repo checks verify adoption
- [ ] 5.4 Update research artifact's Improvement directions section to reference the shipped change; docs page for the new record + ordering
