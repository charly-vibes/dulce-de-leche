# Tasks: fix-agent-instrumentation-check

Tracked as `DDL-ehu` in beads (base_commit `0b4d771`); the decision record is
`DDL-bn4`; the fleet sweep is `DDL-myf`; the skills decision is `DDL-to6`
(prerequisite of `DDL-ehu`).

## 1. Red — layout fixtures and expectations

- [ ] 1.1 Add fixture builders for the four instrumentation layouts: legacy
      (`openspec/project.md` + `openspec/AGENTS.md` + legacy block), artifact
      (`openspec/config.yaml`), hybrid (both), absent (no `openspec/`).
- [ ] 1.2 Add tests asserting current findings per layout — the legacy and
      absent cases fail today for the artifact layouts (they report "missing
      managed block for openspec").
- [ ] 1.3 Add a scope test: repo with `openspec/` but no pin (genesis case)
      is still checked; repo with neither dir nor pin is not checked.

## 2. Green — instrumentation table

- [ ] 2.1 Introduce `Instrumentation::{Block, Artifact}` in
      `src/diagnostics/repo_checks.rs`; move the existing four entries onto
      it (ddl, wai, beads unchanged; openspec becomes an artifact).
- [ ] 2.2 Implement the scope predicate `tool_dir_exists || pinned` and route
      every entry through it.
- [ ] 2.3 Implement layout classification for the openspec artifact:
      `config.yaml` ⇒ pass; `project.md` only ⇒ legacy advisory;
      both ⇒ hybrid advisory; neither ⇒ missing-artifact warn.
- [ ] 2.4 Replace the fix hints: openspec missing ⇒ the migration command;
      legacy/hybrid ⇒ the migration advisory. `ddl init` is no longer named
      for openspec.
- [ ] 2.5 Keep exit codes and severity contract unchanged (warn, never fail).

## 3. Init contract

- [ ] 3.1 Delete the openspec `AGENT_BLOCKS` assumption from
      `src/installer/init.rs` comments; record why `openspec init --tools
      none` stays the invocation and why `openspec update` must not run from
      the cascade.
- [ ] 3.2 Add a regression test: the cascade does not invoke `openspec
      update` (fake-`openspec` binary on PATH recording argv).
- [ ] 3.3 `tests/init_canonical.rs`: remove openspec from `BLOCK_WRITERS`
      and add a comment naming the 1.0.0 retirement (PR #578), replacing the
      "1.14.2 dropped" wording.

## 4. Docs

- [ ] 4.1 `docs/standardization.md` §4.1: correct genesis's
      `ddl.repo.blocks` row.
- [ ] 4.2 Add the subsection "instrumentation is published by the tool,
      checked by presence" — table of blocks vs artifacts, the ownership
      rule, and the retirement rule.
- [ ] 4.3 `CHANGELOG.md` entry naming the behavior change for `ddl doctor`
      consumers.

## 5. Verify

- [ ] 5.1 `just ci` green, including the rewritten doctor-clean contract.
- [ ] 5.2 Run `ddl doctor` in `sunshine` (artifact layout): the openspec
      warning is gone, nothing else changes.
- [ ] 5.3 Run `ddl doctor` in `genesis` (project.md layout, no pin): a
      legacy advisory appears instead of a missing-block warning.
- [ ] 5.4 Run `ddl doctor` in `dont` (legacy layout, block present): pass with
      a legacy advisory naming `openspec update`.

## 6. Follow-ups (not this change)

- [ ] 6.1 File the fleet sweep (`openspec update` across the nine legacy
      repos) as its own issue; it is a per-repo explicit migration, not part
      of this code change.
- [ ] 6.2 File the skills decision (`openspec init --tools agents --scope
      project`) as a gated change with the committed-churn cost stated.