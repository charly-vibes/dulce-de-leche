<!-- OPENSPEC:START -->
# OpenSpec Instructions

These instructions are for AI assistants working in this project.

Always open `@/openspec/AGENTS.md` when the request:
- Mentions planning or proposals (words like proposal, spec, change, plan)
- Introduces new capabilities, breaking changes, architecture shifts, or big performance/security work
- Sounds ambiguous and you need the authoritative spec before coding

Use `@/openspec/AGENTS.md` to learn:
- How to create and apply change proposals
- Spec format and conventions
- Project structure and guidelines

Keep this managed block so 'openspec update' can refresh the instructions.

<!-- OPENSPEC:END -->

<!-- WAI:START -->
## PRIMARY OBJECTIVE

Build and maintain **dulce-de-leche (ddl)** — the cross-platform bootstrap and
orchestration tool that installs, configures, and updates every tool in the
charly-vibes ecosystem from a single binary. Every action should trace back
to: does this make ddl more reliable across platforms, easier to adopt for
new users, or more comprehensive in tool coverage?

# Workflow Tools

This project uses **wai** to track the *why* behind decisions — research,
reasoning, and design choices that shaped the code. Run `wai status` first
to orient yourself.

## Quick Start

1. `wai sync` — ensure agent tools are projected
2. `wai status` — see active projects, phase, and suggestions

When context reaches ~40%: stop and tell the user — responses degrade past
this point. Recommend `wai close` then `/clear` to resume cleanly.
Do NOT skip `wai close` — it enables resume detection.



## Detailed Instructions

Full workflow reference — session lifecycle, capturing work, command cheat
sheets, cross-tool sync, and PARA structure — lives in **`.wai/AGENTS.md`**.
Read it at the start of your first session or when you need detailed guidance.

## PRIMARY OBJECTIVE (echo)

Build and maintain **dulce-de-leche (ddl)** — the cross-platform bootstrap
and orchestration tool that installs, configures, and updates every tool in
the charly-vibes ecosystem from a single binary. Every action should trace
back to: does this make ddl more reliable across platforms, easier to adopt
for new users, or more comprehensive in tool coverage?

Keep this managed block so `wai init` can refresh the instructions.

<!-- WAI:END -->

## Behavioral Constraints

These constraints are **persistent** — they live outside the WAI managed
block so they survive `wai init`. Do not remove or edit them without
deliberate intent.

### Prohibited (DON'T)

- **DON'T** make breaking changes to the install/update protocol or manifest format without an openspec proposal
- **DON'T** push directly to main — all changes go through feature branches with PR review
- **DON'T** add a new tool without adding it to the manifest and testing on all three platforms (macOS, Linux, Windows)
- **DON'T** hardcode platform-specific paths — use platform detection from genesis/ddl's own detection
- **DON'T** modify managed blocks (`<!-- WAI: -->`, `<!-- OPENSPEC: -->`)
- **DON'T** commit platform-specific build artifacts without testing on at least one non-development platform

### Stop and Ask

Pause and request human input when any of these triggers fire:
1. **Ambiguity** — the ticket text itself is contradictory or underspecified
2. **Scope uncertainty** — the ticket is clear but the change naturally touches code or features not mentioned in it
3. **Irreversibility** — breaking changes to the manifest schema, install paths, or upgrade flow
4. **Secrets/credentials** — any external service, API key, or credential not yet authorized
5. **Test failure persistence** — unresolved test failure after two repair attempts, or the same failure across 3 different approaches
6. **Push/release** — pushing to remote, creating a release, or deploying
7. **Context saturation** — context approaching ~40%; recommend `wai close` then `/clear`

### Minimal Footprint

- Prefer small, focused changes over large refactors — one ticket, one concern
- Delete unused code, don't leave commented-out code behind
- Keep PRs under 400 lines changed. If you cannot, split the work into multiple PRs before proceeding.
- Use existing abstractions (genesis, suite patterns) before introducing new ones
- ddl is an orchestrator — prefer subprocess protocol over shared library linking to keep release cycles decoupled

### Drift Detection

Proceed without routine confirmation when the next step is clear.
Do not ask to continue, fix, or commit — just do it. After each major
action (edit, test run, commit), pause and self-check:
1. **ALIGNMENT** — does this still serve one-binary-one-command bootstrapping?
2. **SCOPE** — did I stay within the ticket scope or did I expand into unticketed work?
3. **FOOTPRINT** — did I leave dead code, debug prints, or unnecessary changes?
4. **GOVERNANCE** — did I follow openspec workflow for spec changes?

If any check fails: undo the last change (`git checkout -- <files>` for
uncommitted edits, `git revert HEAD` for committed) before proceeding,
or open a follow-up ticket.
<!-- WAI:REFLECT:REF:START -->
## Accumulated Project Patterns

Project-specific conventions, gotchas, and architecture notes live in
`.wai/resources/reflections/`. Run `wai search "<topic>"` to retrieve relevant
context before starting research or creating tickets.

> **Before research or ticket creation**: always run `wai search "<topic>"` to
> check for known patterns. Do not rediscover what is already documented.
<!-- WAI:REFLECT:REF:END -->

<!-- BEGIN BEADS INTEGRATION v:1 profile:minimal hash:7510c1e2 -->
## Beads Issue Tracker

This project uses **bd (beads)** for issue tracking. Run `bd prime` to see full workflow context and commands.

### Quick Reference

```bash
bd ready              # Find available work
bd show <id>          # View issue details
bd update <id> --claim  # Claim work
bd close <id>         # Complete work
```

### Rules

- Use `bd` for ALL task tracking — do NOT use TodoWrite, TaskCreate, or markdown TODO lists
- Run `bd prime` for detailed command reference and session close protocol
- Use `bd remember` for persistent knowledge — do NOT use MEMORY.md files

**Architecture in one line:** issues live in a local Dolt DB; sync uses `refs/dolt/data` on your git remote; `.beads/issues.jsonl` is a passive export. See https://github.com/gastownhall/beads/blob/main/docs/SYNC_CONCEPTS.md for details and anti-patterns.

## Session Completion

**When ending a work session**, you MUST complete ALL steps below. Work is NOT complete until `git push` succeeds.

**MANDATORY WORKFLOW:**

1. **File issues for remaining work** - Create issues for anything that needs follow-up
2. **Run quality gates** (if code changed) - Tests, linters, builds
3. **Update issue status** - Close finished work, update in-progress items
4. **PUSH TO REMOTE** - This is MANDATORY:
   ```bash
   git pull --rebase
   git push
   git status  # MUST show "up to date with origin"
   ```
5. **Clean up** - Clear stashes, prune remote branches
6. **Verify** - All changes committed AND pushed
7. **Hand off** - Provide context for next session

**CRITICAL RULES:**
- Work is NOT complete until `git push` succeeds
- NEVER stop before pushing - that leaves work stranded locally
- NEVER say "ready to push when you are" - YOU must push
- If push fails, resolve and retry until it succeeds
<!-- END BEADS INTEGRATION -->
