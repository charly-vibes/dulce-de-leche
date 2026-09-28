# Implementation Status

Current state: **v0.5.0 shipped** — all planned phases are implemented and
published; macOS, Linux, and Windows are verified end-to-end by the
3-platform smoke gate.

## Phases

| Phase | Description | Status |
|-------|-------------|--------|
| Core CLI | Cargo crate, CLI structure, error types | ✅ Done |
| Platform detection | OS, arch, package manager detection | ✅ Done |
| Installation chain | Binary-first, cargo fallback, npm for incitaciones | ✅ Done |
| Init command | Interactive and non-interactive bootstrap | ✅ Done |
| .ddl/ directory | Manifest, symlink farm, gitignore | ✅ Done |
| Status command | Cross-tool health overview | ✅ Done |
| Doctor command | Diagnostics and auto-fix | ✅ Done |
| Version management | Manifest, version, upgrade | ✅ Done |
| Cross-platform release | GitHub Actions, binary builds | ✅ Done |
| 3-platform smoke gate | End-to-end verification of release binaries | ✅ Done |
| Homebrew formula | Formula in homebrew-charly tap (manual installs) | ✅ Done |
| Scoop manifest | Manifest in scoop-charly bucket (manual installs) | ✅ Done |
| Documentation | mdBook, README, help text | ✅ Done |

## Notes

- Since v0.5.0 ([DDL-ei3](https://github.com/charly-vibes/dulce-de-leche/issues/DDL-ei3)),
  brew/scoop are **not** part of ddl's install decisions — installs are
  binary-first with cargo fallback; the tap and bucket serve manual installs.
- Upstream coverage gaps: dont has a placeholder tap formula, so it
  requires a Rust toolchain on machines without release binaries. `bd` has
  no cargo fallback at all (the `beads` crate on crates.io is unrelated) —
  it installs strictly from release binaries.

## Legend

- ✅ Done — completed and tested
- 🔄 In progress — actively being worked on
- ⬜ Not started — design complete, not implemented
