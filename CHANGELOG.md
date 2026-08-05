# Changelog

All notable changes to dulce-de-leche (ddl) are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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