# Design: fix-agent-instrumentation-check

## Evidence baseline

All rows reproduced locally with a fresh repo, non-TTY, `openspec init
--tools none`:

- 0.19.0 writes the `OPENSPEC:START` block and `openspec/AGENTS.md`.
- 1.0.0 and later write neither; 1.14.2+ create `openspec/config.yaml`.
- `openspec update` (2.1.0) on an authentic 0.19 block removes the markers
  and keeps surrounding prose; a block whose body does not match a released
  template survives; `openspec/AGENTS.md` is not deleted.
- `openspec update` on a repo with committed skills reports "Updated: Other /
  Universal (shared .agents skills)" and refreshes those files; on a repo
  without them it reports the machine's global skills.
- `openspec init --tools <id>` on 2.1 without `--scope` writes the machine's
  global `~/.config/agents/skills`; `--scope project` keeps them in the repo.
- `openspec init --help` on 2.0.0 has no `--scope` flag; 2.1.0 does.
- ddl's init cascade skips any tool already recorded in
  `.ddl/manifest.json` (`is_inited`), so "ddl strips the block on every run"
  is false; the exposure is repos with no manifest (sunshine, wai, genesis
  carry no `.ddl/`) or an uncommitted one.

## Decision

Option (b) of DDL-bn4, made concrete: **the check verifies what the tool
publishes; ddl does not expect another tool to reproduce a retired artifact.**

Why not (a) "ddl writes the OPENSPEC block itself": the block is upstream's
retired artifact. Its removal is the supported migration, its content
(`Always open @/openspec/AGENTS.md`) describes a 0.x layout, and any copy
ddl writes would either be deleted by `openspec update` (authentic text) or
survive as a fork of upstream instructions (custom text). Two writers
maintaining one region of `AGENTS.md` is exactly the failure this change
ends.

Why not (c) "drop the requirement silently": openspec presence is still a
real property worth reporting — a repo with `openspec/` but no
`config.yaml` and no `project.md`, or a hybrid carrying both, is
mid-migration and worth an advisory.

## Data model

```rust
enum Instrumentation {
    /// The tool owns a delimited AGENTS.md region and rewrites it.
    Block { start: &'static str, end: &'static str, writer: &'static str },
    /// The tool publishes an artifact; its presence is the signal.
    Artifact {
        path: &'static str,
        legacy: Option<(&'static str, &'static str)>, // (path, fix hint)
        fix: &'static str,
    },
}
```

Entries:

| tool | kind | signal | scope predicate |
|---|---|---|---|
| ddl | Block | `<!-- DDL:START -->` | always (ddl owns the repo) |
| wai | Block | `<!-- WAI:START -->` | `.wai/` exists or pinned |
| beads | Block | `BEGIN BEADS INTEGRATION` | `.beads/` exists or `bd` pinned |
| openspec | Artifact | `openspec/config.yaml` | `openspec/` exists or pinned |

Result levels: `pass`; `warn` + `with_fix` for a missing artifact/block;
`warn` + advisory detail for legacy/hybrid layouts (fix = the migration
command). Missing legacy + missing artifact on a scoped repo is a `warn`,
never a `fail` — the exit-code contract of `ddl doctor` is unchanged.

## Scope predicate

`scoped(tool) = tool_dir_exists || pinned(tool)`, evaluated per repo. This is
the correction to the first draft of this proposal: a pins-only predicate
would drop `genesis` (uses openspec, pins none) and a universal predicate
already misclassifies `sunshine` and `genesis`.

## Edge cases

- **Hybrid** (`project.md` + `config.yaml`): pass on the artifact, advisory
  on the stale `project.md`; the message names the precedence (`config.yaml`
  wins) so an agent is never pointed at the older file.
- **openspec absent from PATH**: report the artifact state, and state that
  the tool is not installed; do not emit a fix hint that cannot run.
- **`.agents/skills` is a symlink**: resolve and require the target inside
  the repo before reporting anything about skills (future-proofing for the
  deferred skills change).
- **2.0 vs 2.1**: no ddl code emits `--scope` in this change. If the skills
  change lands, it must be version-conditional (2.1+) or require 2.1.
- **Windows link stubs**: irrelevant here (no skills check), noted for the
  deferred change.

## Rollback

Revert this change as a unit: the check, the test contract, and the docs
update are one commit. No repo state is migrated by it, so no data rollback
is needed. Pins stay at 0.19.0 until the separate pin change lands.

## Verification

- Unit tests for each table row and each layout state (legacy, artifact,
  hybrid, absent, tool-not-installed).
- `test_init_gates_leaves_doctor_repo_checks_clean`: openspec removed from
  `BLOCK_WRITERS`; assert doctor is clean on a stock repo for the remaining
  entries with the pinned tools on PATH.
- Fixture repos built from the four layouts above, run through `ddl init
  --yes --no-install --gates` then `ddl doctor --json`; assert zero
  `ddl.repo.*` warnings except the intended legacy advisories.
- Live check in `sunshine` (artifact layout): the current
  "missing managed block for openspec" warning disappears; no other
  `ddl.repo.*` finding changes.