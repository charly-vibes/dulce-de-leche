# Ecosystem Development

How to set up and work on the charly-vibes tool ecosystem as a whole. For
hacking on ddl itself, see [Development](./development.md). The conventions
referenced here are normative in
[`docs/standardization.md`](https://github.com/charly-vibes/dulce-de-leche/blob/main/docs/standardization.md).

## The map

Nine in-org tool repos, one meta-tool (ddl, this repo), and three external
consumers:

| Repo | Binary | What it is | Category |
|---|---|---|---|
| [wai](https://github.com/charly-vibes/wai) | `wai` | Workflow management for agent sessions (PARA tracking, session close/next) | Core |
| [testaruda](https://github.com/charly-vibes/testaruda) | `testaruda` | Language-agnostic test selection engine | Core |
| [dont](https://github.com/charly-vibes/dont) | `dont` | Forces agents to ground claims before asserting them | Recommended |
| [pretender](https://github.com/charly-vibes/pretender) | `pretender` | Code quality CLI (complexity, duplication, mutation) | Recommended |
| [vampiro](https://github.com/charly-vibes/vampiro) | `vampiro` | Composition-analysis CLI | Extension |
| [specodelic](https://github.com/charly-vibes/specodelic) | `specodelic` / `spk` | Markdown spec format (Intent/Constraints/Model/Properties) + linter/compiler | Extension |
| [incitaciones](https://github.com/charly-vibes/incitaciones) | — | Skills package (npm; the one non-Rust tool) | Extension |
| [espectacular](https://github.com/charly-vibes/espectacular) | `ah` | Behavioral verification layer | Recommended |
| [whisper](https://github.com/charly-vibes/whisper) | `turu` (aliases: `whisper`, `turututu`) | Deterministic knowledge workspace management | Recommended |
| [dulce-de-leche](https://github.com/charly-vibes/dulce-de-leche) | `ddl` | This orchestrator | — |

External (install-only, not developed here): `bd` (beads, issue tracking),
`openspec` (spec-driven proposals).

Every tool's *why* and *maturity* lives in its README top block and
`docs/src/status.md` — that placement is the standard (§5), not a convention.

## Workspace layout

Sibling checkouts matter: ddl's conformance drift test
(`tests/standard_conformance.rs`) discovers them as `../<repo>/` relative to
this repo, and cross-repo work (registry updates, version bumps) assumes the
flat layout:

```
charly/
├── dulce-de-leche/   # this repo
├── wai/
├── testaruda/
├── dont/  pretender/  vampiro/  specodelic/  incitaciones/  espectacular/
└── homebrew-charly/  scoop-charly/   # taps, updated by release workflows
```

If you keep repos elsewhere, symlink them into the same parent directory —
the drift test skips absent siblings (fleet conformance is a local/ddl-side
check; each repo's own CI only validates itself).

## One-time setup

```bash
# 1. Rust + just + mdBook
rustup component add rustfmt clippy
brew install just mdbook        # or cargo install just / mdbook

# 2. Ecosystem tools used by the repos' own CI (dogfooding)
cargo install wai testaruda pretender dont bd openspec

# 3. Clone the siblings you'll touch
for r in wai testaruda dont pretender vampiro specodelic espectacular incitaciones whisper; do
  git clone "git@github.com:charly-vibes/$r.git" "../$r"   # from ddl/
done

# 4. Per-repo bootstrap + verify
cd ../wai && just setup && just ci    # repeat per repo
```

## Day-to-day

- **Task tracking:** `bd` per repo (issues live in each repo's
  `.beads/issues.jsonl`, committed to git). Cross-repo epics live in ddl.
- **Session lifecycle:** `wai close` / `wai next` per the workspace AGENTS.md.
- **Design changes:** openspec proposal in the affected repo before code.
- **Full check before pushing:** `just ci` — same command CI runs. There is
  exactly one entry point per repo; if a check isn't in `just ci`, it isn't a
  gate.

## Conformance

`cargo test -p dulce-de-leche --test standard_conformance` checks every
present sibling (and ddl itself) against the mechanically-checkable standard
invariants: the three workflow files (§1), the binary release matrix,
root `book.toml` with the charly theme fields (§2/§3), README
motivation+status block (§5), and the pinned-versions file (§4). Red means
that repo needs a rollout fix — file it under the standardization epic
(`DDL-u8x`) and turn one assertion green per commit.

## Releases

Every repo releases the same way (§1):

```bash
git tag v0.X.Y && git push --tags
```

CI then builds all five targets (linux/darwin amd64+arm64 tar.gz, windows
amd64 zip), publishes a GitHub Release with `checksums.txt`, publishes to
crates.io (tag must match `Cargo.toml`), and updates the homebrew tap + scoop
bucket. Caveat: tap/scoop steps skip silently if the `TAP_GITHUB_TOKEN`
secret is absent — re-run the publish job after adding it.

`incitaciones` is the exception: npm via its `npm-publish.yml` (the release
slot under that name).

## Adding a new tool

Follow the onboarding checklist in
[`docs/standardization.md` §7](https://github.com/charly-vibes/dulce-de-leche/blob/main/docs/standardization.md):
three workflow files, root `book.toml` + vendored theme, five book pages,
`llms.txt`, README why/status block, `versions.ddl.toml`, a row in ddl's
registry (`src/platform.rs`, enforced by `tests/tool_registry_drift.rs`),
and a dogfood-matrix entry. Document any deviation in the tool's
`docs/src/status.md`.
