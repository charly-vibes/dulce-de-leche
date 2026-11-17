# Proposal: fix-agent-instrumentation-check

## Why

`ddl doctor`'s `ddl.repo.blocks` check requires `<!-- OPENSPEC:START -->` in
`AGENTS.md` and reports "AGENTS.md missing managed block for openspec" with
the fix hint `ddl init` (`src/diagnostics/repo_checks.rs:11-16`). That
requirement is unsatisfiable against every openspec release since 1.0.0:

| openspec | `openspec init --tools none` |
|---|---|
| 0.19.0 (current pin) | writes the block; creates `openspec/{project.md,AGENTS.md}` |
| 1.0.0, 1.5.0 | writes no `AGENTS.md` at all |
| 1.14.2, 1.16.1, 1.17.0 | no `AGENTS.md`; `openspec/{changes,config.yaml,specs}` |
| 2.0.0, 2.1.0 | no `AGENTS.md`; `openspec/{changes,config.yaml,specs}` |

OpenSpec 1.0.0 ("The OPSX Release", PR #578) lists "Config files removed —
tool-specific instruction files (`CLAUDE.md`, `.cursorrules`, `AGENTS.md`,
`project.md`) are no longer generated" as a breaking change: commands became
agent skills, and project-wide facts moved to `openspec/config.yaml`
(`context`, `rules`, `operations`). Upstream's own upgrade path,
`openspec update`, removes authentic legacy blocks from `AGENTS.md`
(fingerprint-gated: blocks whose text matches a released template are
removed; hand-modified text survives) and prints instructions to migrate
`openspec/project.md` into `config.yaml`.

Consequences today, all reproduced against the installed CLI:

1. **Unfixable warning.** On openspec ≥ 1.0, doctor warns, the hint runs
   `openspec init --tools none`, which writes no block, so the warning
   persists. Under the 0.19 pin the warning is latent (init still writes the
   block), which is why CI has not caught it.
2. **No fix right.** On a repo whose block exists, a hand re-created block is
   removed again by the next `openspec update` only when its text matches a
   released template; a custom block survives but is invisible to upstream.
3. **Wrong scope.** `AGENT_BLOCKS` is a universal list, but the fleet already
   has three instrumentation layouts: nine repos carry the legacy block and
   `openspec/AGENTS.md`; `sunshine` carries `openspec/config.yaml` **and**
   `openspec/project.md` (hybrid); `genesis` carries `openspec/project.md`
   and uses openspec for spec authoring while pinning none (see
   `genesis/versions.ddl.toml`: "used for spec authoring but not installed in
   CI"). A pins-only predicate would silently drop genesis.
4. **Wrong owner.** The check hardcodes other tools' output formats. openspec
   is the first writer to change its format; wai and bd are coupled the same
   way.

`test_init_gates_leaves_doctor_repo_checks_clean` records the symptom: it
skips whenever an installed block-writing tool drifts from the pin, because
the contract is really "the installed openspec version writes what ddl
expects", not "this repo is instrumented".

## What Changes

- **Replace the block list with an instrumentation table.** Each entry states
  what the tool publishes and how presence is decided:
  - block entries (ddl, wai, beads) keep their current markers — those tools
    still publish and maintain them;
  - openspec becomes an artifact entry: `openspec/config.yaml` present ⇒
    pass; `openspec/project.md` alone ⇒ advisory "legacy layout: run
    `openspec update` and migrate `project.md` into `config.yaml`";
    both present ⇒ advisory "hybrid layout: migrate, then delete
    `project.md`".
- **Presence-based scope.** A repo is checked for a tool when the tool's own
  directory exists (`openspec/`, `.wai/`, `.beads/`) or the repo pins it in
  `versions.ddl.toml`. Pins alone are insufficient (genesis) and universal
  membership is wrong (not every repo hosts every tool).
- **Retire `OPENSPEC:START` as a pass condition.** A legacy block, where
  present, is accepted with an advisory naming `openspec update`; ddl never
  writes or re-creates it.
- **`ddl init` stays out of the migration.** It keeps running
  `openspec init --tools none` for tools not yet recorded as inited in
  `.ddl/manifest.json`, and MUST NOT run `openspec update` (that command
  rewrites the repo's committed `.agents/skills/openspec-*` files and prints
  tool-detection output, so it is an explicit, pin-driven per-repo step).
- **Doctor details and fix hints** name an action that can succeed: for
  openspec, the hint is the migration command, not `ddl init`.
- **Tests.** `test_init_gates_leaves_doctor_repo_checks_clean` stops
  treating openspec as a block writer; the remaining block writers keep their
  pin check. New scenarios cover legacy, hybrid, artifact-present and
  tool-absent layouts.
- **Docs.** `docs/standardization.md` §4.1 corrects genesis's
  `ddl.repo.blocks` row and gains an "instrumentation is published by the
  tool, checked by presence" subsection.

## Impact

- Affected specs: `health` (new requirement), `bootstrap` (new requirement)
- Affected code: `src/diagnostics/repo_checks.rs`,
  `src/installer/init.rs` (comment + invocation comment only),
  `tests/init_canonical.rs`, `docs/standardization.md`
- Not affected: install protocol, manifest format, `.ddl/` layout, gates
  wiring
- Non-goals: no `openspec update` automation, no global-state mutation, no
  re-creation of a retired block
- Deferred: whether ddl ships openspec skills
  (`openspec init --tools agents --scope project`, 2.1+) as the agent surface
  that replaces the block — tracked as its own gated change, because it makes
  every repo commit six version-tied skill directories.

## Risks

- The advisory window (legacy block accepted) must be time-boxed, or it
  becomes the next fossil. The change records removal when the fleet has no
  `openspec/AGENTS.md` left.
- `openspec/config.yaml` presence alone does not prove the agent surface is
  reachable; that is the skills decision above, deliberately out of scope
  here.