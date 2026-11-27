# Changelog

All notable changes to dulce-de-leche (ddl) are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **`ddl doctor --strict`** — exits non-zero when any repo-hygiene
  diagnostic (`ddl.repo.*`) is at warn/fail level; the full report is
  emitted first, so gate consumers keep the structured evidence.
  Machine-scope findings (tool installs, nested tool doctors) do not fail
  a strict run (`DDL-9kz.2`)
- **repo-local `.gitignore` negation for `.beads/`** — `ddl init --gates`
  appends `!.beads/` + `.beads/embeddeddolt/`, so the tracked export is
  addable without `-f` while the embedded DB stays ignored (`DDL-9kz.2`)
- **three new `ddl.repo.*` doctor checks** — `ddl.repo.hooks-path` flags a
  `core.hooksPath` that points nowhere at error severity (the per-clone
  recurrence class behind DDL-uce: no git hook runs in such a checkout);
  `ddl.repo.just-ci` warns when a repo has no justfile or no `ci` recipe
  (silently opting out of the standard gate); `ddl.repo.readme-frontdoor`
  warns when README.md is missing any §5.1 front-door section, naming all
  misses in one finding
- **session-close protocol block** (DDL-9kz.1) — `ddl init --gates` now
  writes a second ddl-owned managed block (`SESSION-PROTOCOL`) into the
  repo-local AGENTS.md carrying the close ritual verbatim (quality gates,
  `bd close` with evidence, the exact `bd export -o .beads/issues.jsonl &&
  git add -f .beads/issues.jsonl && git commit` line, rebase + `git push`,
  `wai close`), so harnesses that stop at the repo root still see it.
  Idempotent, self-owned (a drifted span is rewritten in place), and
  drift-detectable (`ddl.repo.blocks` flags a missing or drifted span)

- **`podras` joins the managed registry** (16th tool, extension/experimental) —
  per-repository project-status brief for humans and AI agents
  (`charly-vibes/podras`). Capability matrix, README block, ecosystem map,
  troubleshooting list, and the §4 dogfood matrix updated; `ddl install podras`
  works once podras publishes its first crates.io/GitHub Release. ddl
  dogfoods it locally: `BRIEF.md`/`BRIEF.json` committed at the repo root.

### Changed

- **the managed pre-commit pretender gate is armed** — the ddl-managed
  `ddl-pretender-gate` hook now runs `pretender check --staged --mode gate`
  instead of `--diff-only`. `--diff-only` computes the changed-file set from
  the *unstaged* tree, which is empty by construction at pre-commit time
  (everything is already staged), so the gate exited 0 without scanning a
  single file in every ddl-wired repo. `--staged` selects the staged path
  set and scans the **working-tree copy** of each selected path — a commit
  that does not touch files carrying pre-existing violations is not blocked
  (DDL-3fd intent). Known residual gaps: the *staged blob* itself is not
  scanned (index holds the violating version, worktree the clean one —
  index-accurate scanning needs `git show :path` support, tracked as
  DDL-api), and merge commits bypass the gate entirely (git does not run
  pre-commit for `git merge`; pre-merge-commit is not wired). Repos can
  override the command via `.ddl/config.toml` (`gates.pretender_gate`)
  (`DDL-sl9`). **Upgrading repos:** re-run `ddl init --gates` to adopt the
  armed gate. First blocked commit? `pretender check --adopt` adopts every
  current violation into the gate baseline (`pretender-baseline.json`);
  afterwards gate mode fails only on genuinely new findings — without a
  baseline, gate mode fails on any violation (`DDL-3zm`)
- **`ddl init` installs openspec project skills** — on openspec 2.1+ it runs
  `openspec init --tools agents --scope project`; older (2.0 has no `--scope`)
  or unparseable versions keep the artifact-only `init --tools none` with a
  warning, so an older or unscoped install never touches machine-global
  `~/.config/agents/skills`. Fleet pin bumped to 2.1.0 (`DDL-qfv`)
- **`ddl doctor`'s `ddl.repo.blocks` verifies what each tool publishes** —
  managed blocks for ddl/wai/beads, `openspec/config.yaml` for openspec —
  instead of requiring the `OPENSPEC:START` block openspec retired in 1.0.0;
  legacy and hybrid openspec layouts now report a migration warning pointing
  at `openspec update` (`DDL-ehu`)

## [0.10.0] - 2026-11-16

Provenance verification that accepts the fleet's own releases, the `ddl setup`
wizard, and the installer/module decomposition.

### Added

- **`ddl setup` — setup wizard step machine** — pre-flight → migrate → scope →
  delegate → verify, wired to the `setup` subcommand, with JSON failure events
  matching `init`'s emission contract and a TTFIF measurement harness
  (`DDL-g6w.1`–`DDL-g6w.8`)
- **URL-derived provenance identity** — the expected repository is read from
  the archive URL instead of a hardcoded ddl-own constant, so tools published
  from their own repositories verify (`DDL-9x4.1`)
- **Checksums-only channel for external origins** — third-party releases that
  cannot publish `provenance.json` (bd from gastownhall/beads) verify through
  their published `checksums.txt` / per-asset digest as a distinct, weaker
  status; charly-vibes origins still require checksum *and* provenance
  (`DDL-9x4.11.1`)
- **bajan in the managed registry** (`bajan-23s`)
- **Test-runner standard** — `just test` is the testaruda exec loop across the
  DDL repos (`DDL-85p`)
- **Contract bundle refusal law + base-contract cache core** (`wild-xgd`)

### Changed

- **Installer, diagnostics, gates, platform, `dot_ddl`, and commands split into
  modules**, with functions decomposed to the complexity budgets — behavior and
  public entry points preserved
- **Seed pins refreshed** in `versions.ddl.toml` and this repo's CI:
  pretender 0.11.1, wai-cli 2026.11.11, testaruda 0.5.9, openspec 2.0.0,
  bd 1.3.4 (checksum-verified)
- **zip 2 → 9** — every zip 2.x release is yanked on crates.io (#92)
- **CI installs testaruda and pytest 9.0.3** — the `just test` recipe cannot
  run without them

### Fixed

- **Reap child status after pipe EOF** in `run_child_captured`
- **`init` doctor-clean e2e skips with a named drift reason** when PATH tools
  differ from the fleet pins (`DDL-gef`)
- **Troubleshooting tool list matches the registry**, covered by the drift
  test (`DDL-2rn`)
- **`ddl setup` runs the wizard step machine** instead of the placeholder
  (`DDL-g6w.8`)

### Documentation

- Ecosystem standard gains the README front-door contract and the requirement
  that releases ship prebuilt binaries; ddl's README conformance status
  corrected; `llms.txt`/`llm.txt` book sync for the wizard.

## [0.9.0] - 2026-10-16

Security-hardening release (SEC-01 through SEC-07 remediation) plus
init/gates reliability and fleet-standard compliance.

### Added

- **`install.sh` — checksum-verified stable bootstrap** — the curl\|bash path
  verifies a published SHA256 before installing ([DDL-1ay], SEC-03 green)
- **Bounded downloads, extraction, and child execution** — timeouts, size
  limits, and process groups across the install/upgrade paths ([DDL-1z1],
  SEC-02 green)
- **Stage-then-atomically-replace binary upgrades** — no more half-written
  binaries on interrupted upgrades ([DDL-4oi], SEC-01 green)
- **Install lock with artifact identities** — locked replay policy and
  deterministic release-age upgrade policy ([DDL-5bs])
- **Reject release downloads when verification is unavailable** — fail-closed
  instead of silently skipping checksums ([DDL-yts])
- **lefthook template chains bd hooks** — `init` writes the DDL block,
  reconciles the manifest, and scaffolds pins ([DDL-fw7])
- **Beads tracked-export invariant** — new `ddl.repo.beads-export` doctor
  check plus a pre-flip export guard in gates wiring
- **Fleet-standard compliance doctor checks** ([DDL-rod])
- **Managed tool registry growth** — `wild` and `funkalization` (#12,
  experimental); `dot_ddl` tracked check migrated onto `genesis::git`
- **CI recipes** — advisory + secret-scan recipes backing the release
  security gate

### Fixed

- **Atomic manifest replacement with dedicated lock file** ([DDL-8u8],
  SEC-01) and **race-free no-follow manifest opens** closing the
  check-then-open swap window; refuse manifest writes redirected by links
  outside the project ([DDL-sg8], SEC-05)
- **Reject artifacts from unexpected publisher/build identity** ([DDL-wof],
  SEC-03/06)
- **Surface and retry failed channel propagation** ([DDL-9e7], SEC-07)
- **Release credentials scoped** — publish gated on tests + security (SEC-06)
- **init UX** — config-step failures are advisory so gates wiring always
  completes ([DDL-6ox]); skills-scope select only on interactive stdio;
  `bd init` gets a 300s budget for the embedded Dolt bootstrap on loaded CI
  runners
- **Gates** — pre-commit template tolerates nothing-to-lint and gates
  pretender diff-only ([DDL-3fd])

## [0.8.0] - 2026-10-08

### Added

- **`ddl init --gates`** — wires lefthook pre-commit/pre-push managed blocks
  (`ah check`, `pretender check --mode gate`, `spk lint openspec`), `.gitignore`
  entries, and beads no-db mode; idempotent and self-repairing against the
  espectacular managed-block mangle class ([DDL-6zn.2])
- **Versioned structured envelope contract for `doctor`/`status`/`init` JSON**
  — `data = {summary, diagnostics: [{tool, check, level, detail, fix?}]}`;
  summary is recomputed from the items so counts can never contradict them,
  `fix` is omitted-never-null, and informational detail moved out of doctor
  into `ddl status` (envelope_version 0.1, pinned by CI contract tests)
  ([DDL-6zn.4])
- **Repo-aware doctor** — four machine-checkable repo-scope checks
  (`ddl.repo.gates`, `ddl.repo.blocks`, `ddl.repo.gitignore`, `ddl.repo.pins`)
  defining "this repo is ddl-initialized/conformant"; checks register only
  inside a git repo so their presence is the signal ([DDL-6zn.5])
- **Generated help examples + canonical tool alias resolution** —
  install/init help examples are generated from the registry (no more stale
  hand-written lists); one shared resolution entry point accepts canonical
  names and aliases uniformly across `init` and `install`, with guidance when
  ddl self-references are passed as tools ([DDL-6zn.8])

### Fixed

- **Init cascade failures are hard failures** — a failed tool-init step no
  longer exits 0; the run fails with a resume summary naming the exact
  finish command and selective re-run (`ddl init --tools <failed>`), with
  prerequisite ordering (openspec before ah) and idempotent-until-green
  semantics ([DDL-6zn.3])
- **`ddl migrate` git-tracked safety** — phase-1 symlink migration no longer
  corrupts git-tracked config dirs (tracked files no longer show as deleted);
  `--undo` is non-destructive and clears stuck `migration_state: "phase1"`
  residue ([DDL-6zn.7])

### Documentation

- Book documents the `ddl feedback` subcommand ([DDL-6zn.6])

## [0.7.0] - 2026-09-29

### Added

- **`ddl catalog` command** — one command answering "what tools exist, what
  does each do, how to invoke" from the tool registry. Human table grouped
  by adoption category; `--json` emits a genesis `List` envelope. Generated
  `docs/capability-matrix.md` is drift-checked against the registry in CI
  ([DDL-91a])
- **`ddl feedback` subcommand** — files an issue to
  `charly-vibes/dulce-de-leche` via `gh` (`bug|feature|question|chore`);
  piped stdin (multi-line input promotes the first line to the title),
  `--title` override, `--dry-run`, and `--from-last-error`. Failed ddl
  commands are now recorded to the genesis scratch store so
  `--from-last-error` auto-reports the last failure ([DDL-isi])
- **Checksum verification for binary installs** — downloaded release
  binaries are sha256-verified against the release's published
  `checksums.txt` (per-asset `.sha256` fallback) before extraction; a
  mismatch aborts naming the tool, an absent checksums file warns and
  proceeds ([DDL-5ph])
- **Registry partition surfaces adoption tiers** — every managed tool is
  tagged `core`/`recommended`/`extension` plus a maturity level
  (stable/working/tracer-bullet/spec-stage), rendered in `ddl status` and
  version JSON ([DDL-3i4])

### Fixed

- Binary installs no longer record `unknown` (or `vunknown`) as the
  installed version: the just-downloaded binary in `.ddl/bin` is probed
  directly when it is not yet on the current process's PATH ([DDL-x0m])
- `ddl install` on a PATH-but-untracked tool now records a manifest entry
  instead of silently skipping the record ([DDL-zw4])
- CI flake: init tests no longer hit the network for real `npx` skill
  installs — `incitaciones` is stubbed on PATH in tests

### Docs

- `docs/capability-matrix.md` — generated intent → tool table, drift-
  checked against the registry ([DDL-91a])
- openspec specs and project.md synced to the current 11-tool registry;
  stale `fotos-mcp`/`fabbro` references removed ([DDL-gap])

## [0.6.0] - 2026-09-28

### Changed

- **Removed `fotos-mcp` and `fabbro` from the managed toolset** — the fotos
  desktop app repo and the fabbro Go CLI are out of scope for ddl's
  registry, compat matrix, legacy-config migration, and post-install init.
  Their tap formulas remain published for manual installs.

### Added

- **`bd` (beads) joins the managed toolset** — binary download from
  `gastownhall/beads` releases (assets named `beads_*` shipping the `bd`
  binary, via the new `binary_asset_prefix` override). No cargo fallback:
  the `beads` crate on crates.io is an unrelated package, so the cargo
  fallback is now gated on the tool actually having a crates.io package
  (`cargo_installable`). Install-only — bd manages repo-local `.beads/`
  itself; post-install init runs `bd init --non-interactive`.
- **`openspec` joins the managed toolset** — npm-distributed
  (`@fission-ai/openspec`); npm install/version/upgrade paths now use
  `npm_package` so scoped packages work. GitHub releases at
  `fission-ai/openspec` serve the binary-first path. Install-only —
  openspec manages the repo-local `openspec/` directory itself;
  post-install init runs `openspec init --tools none`.
- Corrected specodelic's legacy config mapping: `.specs/` (its spec
  corpus), not `.beads/` (which belongs to bd).

## [0.5.0] - 2026-09-15

### Changed

- Simplified install chain to **binary-first on every platform** with `cargo
  install` as the runtime fallback (when a release binary is unavailable and
  cargo is present); Homebrew and Scoop leave ddl's install decision path
  entirely — the tap and bucket remain published for manual installs
  ([DDL-ei3], openspec `simplify-install-chain`)
- Removed `InstallMethod::Brew`/`InstallMethod::Scoop`, placeholder-formula
  detection, and brew/scoop install paths (dead code after the decision change)

## [0.4.0] - 2026-09-15

### Added

- `whisper` joins the managed toolset as `turu` (`whisper-vibes` crate) —
  install-only; its state is home-dir global (`~/.whisper/`), not repo-local
  config ([DDL-9c2])
- `incitaciones` joins the managed toolset — npm-distributed (`InstallMethod::Npm`),
  install-only like fotos-mcp ([DDL-b33])
- `ddl init` checks for globally installed incitaciones skills (`~/.agents/skills/`,
  detected via the `installed-from: incitaciones` frontmatter marker) and prompts
  to install them globally, locally, or skip when missing; `--yes` installs
  globally, `--json` is check-only ([DDL-b33])
- `ddl doctor` reports incitaciones skill installation state (global, local-only,
  or missing) ([DDL-b33])
- 3-platform smoke gate (macOS, Linux, Windows) validating real release binaries
  end-to-end ([DDL-ei3])

### Changed

- `ddl init` now always runs `wai init` — even when wai was already installed —
  so every init guarantees the `.wai/` PARA structure exists ([DDL-b33])

### Fixed

- Windows: manifest writes now retry through AV file-lock contention with
  exponential backoff, and a single writer handle eliminates the `LockFileEx`
  self-deadlock ([DDL-71i])
- GitHub API calls now send `GITHUB_TOKEN` and surface a distinct rate-limit
  error ([DDL-2rb])

## [0.3.0] - 2026-08-05

### Added

- `--verbose` flag now works: emits trace messages to stderr during install,
  upgrade, and init across all install methods (binary, cargo, brew, scoop)
  ([DDL-02a])
- 33 new integration tests across doctor, install, scope, status, and version
  commands ([DDL-1n7], [DDL-4xc], [DDL-8z4], [DDL-kl3], [DDL-nj1])

### Fixed

- `init --json` now emits multiple separate JSON objects instead of one
  concatenated blob ([DDL-xhy])
- `status`, `doctor`, `version` no longer silently create `.ddl/` directory
  ([DDL-3dq])
- JSON error envelope now shows the actual error message instead of Rust
  Debug format ([DDL-tvm])
- `status_summary` reports the correct installed tool count ([DDL-2o3])
- `migrate` is now idempotent — re-running produces the same result
  ([DDL-40e])
- `doctor --fix` now actually applies the documented fixes ([DDL-9ge])
- Prevent concurrent manifest updates from losing entries ([DDL-h1l])
- Three P0 migrate bugs: single-file config corruption, symlink path
  normalization, edge cases ([DDL-cbp], [DDL-0yp], [DDL-el9])
- P3 bugs: user_agent, banner, fotos-mcp, network warning leak

### Changed

- Adopted genesis v0.6.0 envelope API — all 9 `json_output` call sites
  updated with `cli_version` arg ([DDL-iqv])

### Documentation

- Updated `docs/design.md` and `CLAUDE.md` from "Design phase" to "Implemented"
- Fixed symlink direction in `README.md` and `docs/src/architecture.md`
  (legacy → `.ddl/`, not `.ddl/` → legacy)
- Documented `-v`/`-vv`/`-vvv` progressive verbosity levels
- Fixed exit code table in `docs/src/commands.md` (all errors exit 1)
- Added footnote for `--profile minimal` as a reserved future feature

## [0.2.0] - 2026-07-31

### Added

- Adopted 6 genesis v0.4.0 modules: `guide` (CliVerbosity, CliFormat),
  `cli` (completions, version-json), `config` (DdlConfig), `fixture`
  (test helpers), `aix` (agents block generation)
- Interactive `ddl init` with cliclack multi-select tool picker
- `--yes` non-interactive mode for CI
- `--json` output on all commands (genesis-vibes envelope format)
- Health diagnostics: subprocess calls to each tool's status/doctor
- Version compatibility matrix with dynamic fetch + embedded fallback
- Cross-platform CI: fmt + lint + test + build on push
- Release workflow: cross-compile 5 platforms → crates.io → Homebrew → Scoop
- Docs workflow: mdBook → GitHub Pages

### Fixed

- Homebrew/Scoop placeholder detection before install

## [0.1.0] - 2026-07-30

### Added

- Initial crate scaffold with CLI (clap derive)
- Platform detection (macOS, Linux, Windows, ARM/AMD64)
- Binary download, cargo install, brew install, scoop install
- `ddl init`, `ddl install`, `ddl status`, `ddl doctor`, `ddl version`,
  `ddl upgrade`, `ddl migrate`, `ddl scope` commands
- Dot-ddl directory management (create, find, migrate, undo)
- Error types with miette diagnostics
- 21 tests, CI pipeline

[DDL-02a]: https://github.com/charly-vibes/dulce-de-leche/issues/DDL-02a
[DDL-1n7]: https://github.com/charly-vibes/dulce-de-leche/issues/DDL-1n7
[DDL-4xc]: https://github.com/charly-vibes/dulce-de-leche/issues/DDL-4xc
[DDL-8z4]: https://github.com/charly-vibes/dulce-de-leche/issues/DDL-8z4
[DDL-kl3]: https://github.com/charly-vibes/dulce-de-leche/issues/DDL-kl3
[DDL-nj1]: https://github.com/charly-vibes/dulce-de-leche/issues/DDL-nj1
[DDL-xhy]: https://github.com/charly-vibes/dulce-de-leche/issues/DDL-xhy
[DDL-3dq]: https://github.com/charly-vibes/dulce-de-leche/issues/DDL-3dq
[DDL-tvm]: https://github.com/charly-vibes/dulce-de-leche/issues/DDL-tvm
[DDL-2o3]: https://github.com/charly-vibes/dulce-de-leche/issues/DDL-2o3
[DDL-40e]: https://github.com/charly-vibes/dulce-de-leche/issues/DDL-40e
[DDL-9ge]: https://github.com/charly-vibes/dulce-de-leche/issues/DDL-9ge
[DDL-h1l]: https://github.com/charly-vibes/dulce-de-leche/issues/DDL-h1l
[DDL-cbp]: https://github.com/charly-vibes/dulce-de-leche/issues/DDL-cbp
[DDL-0yp]: https://github.com/charly-vibes/dulce-de-leche/issues/DDL-0yp
[DDL-el9]: https://github.com/charly-vibes/dulce-de-leche/issues/DDL-el9
[DDL-iqv]: https://github.com/charly-vibes/dulce-de-leche/issues/DDL-iqv
[DDL-b33]: https://github.com/charly-vibes/dulce-de-leche/issues/DDL-b33
[DDL-9c2]: https://github.com/charly-vibes/dulce-de-leche/issues/DDL-9c2
[DDL-2rb]: https://github.com/charly-vibes/dulce-de-leche/issues/DDL-2rb
[DDL-71i]: https://github.com/charly-vibes/dulce-de-leche/issues/DDL-71i
[DDL-ei3]: https://github.com/charly-vibes/dulce-de-leche/issues/DDL-ei3[DDL-91a]: https://github.com/charly-vibes/dulce-de-leche/issues/DDL-91a
[DDL-isi]: https://github.com/charly-vibes/dulce-de-leche/issues/DDL-isi
[DDL-5ph]: https://github.com/charly-vibes/dulce-de-leche/issues/DDL-5ph
[DDL-3i4]: https://github.com/charly-vibes/dulce-de-leche/issues/DDL-3i4
[DDL-x0m]: https://github.com/charly-vibes/dulce-de-leche/issues/DDL-x0m
[DDL-zw4]: https://github.com/charly-vibes/dulce-de-leche/issues/DDL-zw4
[DDL-gap]: https://github.com/charly-vibes/dulce-de-leche/issues/DDL-gap
[DDL-6zn.2]: https://github.com/charly-vibes/dulce-de-leche/issues/DDL-6zn.2
[DDL-6zn.3]: https://github.com/charly-vibes/dulce-de-leche/issues/DDL-6zn.3
[DDL-6zn.4]: https://github.com/charly-vibes/dulce-de-leche/issues/DDL-6zn.4
[DDL-6zn.5]: https://github.com/charly-vibes/dulce-de-leche/issues/DDL-6zn.5
[DDL-6zn.6]: https://github.com/charly-vibes/dulce-de-leche/issues/DDL-6zn.6
[DDL-6zn.7]: https://github.com/charly-vibes/dulce-de-leche/issues/DDL-6zn.7
[DDL-6zn.8]: https://github.com/charly-vibes/dulce-de-leche/issues/DDL-6zn.8
