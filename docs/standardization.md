# Ecosystem Standardization Proposal

> **Status:** RATIFIED v0.3 (2026-09-30, bd `DDL-u8x`). Rollout is tracked in bd under this epic:
> Applies to every in-org tool repo managed by `ddl`: wai, testaruda, dont, pretender,
> vampiro, specodelic, incitaciones, espectacular (the `ah` binary), whisper (the `turu`
> binary), and dulce-de-leche itself — plus any future charly-vibes CLI tool. External
> consumers (`bd`, `openspec`) appear in the dogfood matrix but are not rollout targets.
> `microdancing` (blog), `jams`, and non-CLI artifacts are out of scope except as the
> source of the visual theme.

## 0. Principles

1. **One way to evaluate a tool:** clone it, run `just ci`, read `README.md`
   top block, read `docs/`. Every repo answers *why does this exist* and *how
   mature is it* in the same place, the same way.
2. **Dogfood by default:** every tool is exercised by the rest of the
   ecosystem in its own CI. A tool that the ecosystem doesn't use on itself
   must say why in its status page.
3. **Rust-CLI shaped:** the standard assumes Cargo + `just` + mdBook. Tools
   that genuinely differ (e.g. `incitaciones`, a skill/pnpm package) follow
   the closest applicable sections and must document the deviation in their
   status page.

## 1. CI standard

Every tool repo has exactly three workflow files (plus tool-specific extras
only when unavoidable):

| File        | Jobs                                                                     |
|-------------|--------------------------------------------------------------------------|
| `ci.yml`    | single job `ci`, name `Test & Build`, `timeout-minutes: 10`, runs `just ci` |
| `docs.yml`  | builds the mdBook from `book.toml`, deploys to gh-pages                  |
| `release.yml` | tag-triggered build + crates.io / brew / scoop / npm publish as applicable |

Binary release matrix — every Rust tool repo (all except `incitaciones`, which
publishes to npm instead) must publish **all five targets**:

| OS      | Arch  | Target triple                | Archive   |
|---------|-------|------------------------------|-----------|
| linux   | amd64 | `x86_64-unknown-linux-gnu`   | `.tar.gz` |
| linux   | arm64 | `aarch64-unknown-linux-gnu`  | `.tar.gz` |
| darwin  | amd64 | `x86_64-apple-darwin`        | `.tar.gz` |
| darwin  | arm64 | `aarch64-apple-darwin`       | `.tar.gz` |
| windows | amd64 | `x86_64-pc-windows-msvc`     | `.zip`    |

Naming: `{tool}_{VERSION}_{os}_{arch}` (e.g. `ddl_0.4.0_linux_arm64.tar.gz`),
plus a single `checksums.txt` (`sha256sum`) covering all archives — this is the
canonical scheme seven repos already follow. After publishing, release.yml must
auto-update the `homebrew-charly` tap and `scoop-charly` bucket (ddl's release
workflow is the reference implementation). Current conformance: 7/8 conform;
`specodelic` has no `release.yml` (crates.io only) — tracked for rollout.

Rules:

- `ci.yml` steps, in order: `Checkout` → `Set up Rust` (or Node) → `Cache cargo`
  → `Install just` → *dogfood installs (see §4)* → `Run CI pipeline` (`just ci`).
- No repo invents its own job names. `dont`'s `quality` job and `vampiro`'s
  `pages.yml`/`planning` job are migrated onto the standard names.
- `incitaciones` gains a `ci.yml` (lint + link-check of skills/content); its
  existing `npm-publish.yml` is the §1 `release.yml` slot under that name
  (registry-specific publish workflows map to the release slot).
- README badges must mirror actual workflow files — a badge for a workflow
  that doesn't exist is a bug.

## 2. Documentation structure

```
book.toml            # repo root
docs/
  src/
    SUMMARY.md       # mdBook TOC
    specs/           # OpenSpec specs copied here at build time (never committed)
    index.md         # what it is, 30-second pitch, install
    getting-started.md
    configuration.md
    cli.md           # every command, every flag
    status.md        # §5 status page (see below)
  adr/               # architecture decision records (optional)
  research/          # raw research, evaluations, JSON artifacts (never in src/)
llms.txt             # root; llm-oriented summary
llm.txt
```

Rules:

- `docs/src/` contains only book pages. Generated artifacts (validation JSON,
  stress-test candidate dumps, review reports) move to `docs/research/`.
- `book.toml` must live at the **repo root** (ddl currently keeps it at `docs/book.toml`
  and migrates first — the standard's home dogfoods the standard). It must set:
  `title` (tool name, lowercase), `authors = ["charly vibes"]`, `language = "en"`,
  `description`, `default-theme = "coal"` (mdBook only supports built-in theme names
  for `default-theme`; `additional-css` restyles coal into the charly theme — see §3),
  and `additional-css = ["theme/charly.css"]`.
- Every tool ships `llms.txt` at root.
- OpenSpec specs deploy into the book: `docs.yml` copies
  `openspec/specs/*/spec.md` → `docs/src/specs/<name>.md` at build time (or a
  docs-assembly script generates them, e.g. vampiro's `build_docs.py`), and
  `docs/src/SUMMARY.md` links every spec page under a `# Design Specs`
  heading. Copies are build artifacts — never committed; raw copies without
  SUMMARY links do not render. Enforced by the `s2_specs_deployed_in_docs`
  conformance test.
- README badge row is standardized (order fixed): wai-tracked → CI → Release
  → Docs → registry (crates/brew/scoop/npm) → docs.rs → License.

## 3. Documentation theme — "microdancing"

New shared mdBook theme derived from `microdancing/templates/assets/style.css`.
Shipped as a single vendored file per repo at `theme/charly.css` (small, no
build step, survives mdBook upgrades — no theme crate/repo needed). It restyles
the built-in `coal` theme (set as `default-theme`); mdBook cannot register a
selectable custom theme name without vendoring a full `theme/index.hbs`, which
we avoid.

Palette (from microdancing CSS custom properties):

| Token      | Dark (default, restyles `coal`) | Light (restyles `light`) |
|------------|---------------|-------|
| `--bg`     | `#1e1c32` (card-bg) | `#82B1FF` |
| `--bg-alt` | `#2C2A4A` | `#F5F5F5` (card-bg) |
| `--fg`     | `#EAE1DF` | `#2D1B14` |
| `--accent` (links, highlights) | `#FF007F` | `#E91E63` |
| `--highlight` | `#FFC107` | `#FFC107` |
| `--muted`  | `#B392AC` | `#6b5d58` |
| `--ok`     | `#2E7D32` | `#2E7D32` |

Dark is the default (`default-theme = "coal"` restyled by the vendored CSS); the
light theme reuses the same tokens. Applied per-repo by vendoring
`theme/charly.css` — no submodule, no network fetch in CI.

## 4. Dogfooding matrix

Every in-org tool repo's CI installs and runs a defined subset of the ecosystem
on itself. **Installs are pinned:** each repo commits a `versions.ddl.toml`
(checked by the conformance test) and installs via that pin, never `latest` —
a broken release of any tool must not break CI of the whole fleet.

Rows are in-org repos only; external consumers (bd, openspec, turu) appear as
**columns** and are installed only where a repo genuinely consumes them.

| Consumer ↓ runs | pretender (lint/complexity) | wai (workflow) | bd (issues) | openspec (proposals) | dont (grounding) | testaruda (test selection) | ddl (status) |
|---|---|---|---|---|---|---|---|
| wai          | ✅ (already) | — | ✅ | ✅ | ◻ opt | — | ✅ |
| testaruda    | ✅ (has pretender.toml) | ✅ | ✅ | ✅ | ◻ opt | — | ✅ |
| dont         | ✅ | ✅ (already) | ✅ | ✅ | — | ◻ opt | ✅ |
| pretender    | — | ✅ | ✅ | ✅ | ◻ opt | ◻ opt | ✅ |
| vampiro      | ✅ | ✅ | ✅ | ✅ (already) | ✅ (already) | ◻ opt | ✅ |
| specodelic   | ✅ | ✅ | ✅ | ✅ | ◻ opt | — | ✅ |
| incitaciones | ✅ | ✅ | ✅ | — | ◻ opt | — | ✅ |
| espectacular (`ah`) | ✅ | ✅ | ✅ | ✅ | ◻ opt | ◻ opt | ✅ |
| whisper (`turu`) | ✅ | ✅ | ✅ | — | ◻ opt | — | ✅ |
| ddl          | ✅ | ✅ | ✅ | ✅ | ◻ opt | — | — |

`✅` = required in `ci.yml` (unless the section says otherwise); `◻ opt` = opt-in;
`—` = self/not applicable.
`ddl status` is the runtime check: the registry drift test enforces the
README block, and each tool's status page links back to ddl.

## 5. Motivation & status block (the "why / how mature")

Every tool README starts with the standard block, immediately after the
title/tagline and before badges:

```markdown
> **Why:** <one paragraph — the problem, who has it, why existing tools fail>
> **Status:** [stable|beta|experimental|sunset](docs/status.md) · <one-line> · [Motivation & design](docs/src/index.md)
```

Status definitions:

- **stable** — public API frozen; semver; breaking changes only via major.
- **beta** — core works, API may still shift; used in anger by ≥1 other
  ecosystem tool.
- **experimental** — exploratory; may be renamed or sunset.
- **sunset** — maintenance mode; no new features. CI shrinks to build+test only
  (no new dogfood requirements); docs frozen except status page.

Transitions are proposed by the tool's owner via an openspec change in the
tool's repo and recorded in ddl's capability matrix in the same commit.

`docs/src/status.md` standardizes on the pretender table format (command ×
status × notes), plus sections: *Implemented*, *In progress*, *Mapped to
specs* (dont's format), and a *Dogfooding* section listing which ecosystem
tools consume this one and how.

## 6. Rollout order (each step = separate bd issues, red→green where testable)

0. **ddl dogfoods first**: move `docs/book.toml` → root `book.toml`, vendor
   `theme/charly.css`.
1. **ddl**: ratify this doc; add a `tests/standard_conformance.rs` drift test
   that checks sibling repos **when present locally** (fleet conformance is a
   ddl-side/local check; each repo's own CI only validates itself — the test
   skips absent siblings rather than failing). It checks the §1–§5 invariants
   that are mechanically checkable (workflow files exist, book.toml fields,
   README block present, badge↔workflow match, `versions.ddl.toml` present).
   TDD: write the test first against current repos — it will fail; each fix
   turns one assertion green.
2. **Theme**: write `theme/charly.css`, dogfood it in ddl's own book first.
3. **CI migration**: dont, vampiro, incitaciones (worst offenders) → then
   badge fixes everywhere.
4. **Docs restructure**: move artifacts to `docs/research/`, add missing
   `llms.txt`, backfill `status.md` per §5 for all 8 tools.
5. **Dogfooding matrix**: add missing ✅ entries repo by repo.
6. **Statuses**: owner assigns stable/beta/experimental per tool in ddl's
   capability matrix; README blocks updated in the same commit.

## 7. New-tool onboarding checklist

A new charly-vibes CLI tool is conformant on day one when it has: the three
workflow files (§1) with `just ci`; root `book.toml` + `theme/charly.css` + the
five §2 book pages; `llms.txt` + `llm.txt`; the §5 README block; a
`versions.ddl.toml` pin file; a row in ddl's registry + capability matrix
(enforced by `tests/tool_registry_drift.rs`); and an entry in the §4 dogfood
matrix. Deviations must be documented in the tool's `docs/src/status.md`.
