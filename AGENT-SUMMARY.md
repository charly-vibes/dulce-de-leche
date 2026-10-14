# AGENT-SUMMARY — DDL-sg8: Prevent repository manifest links from redirecting writes outside the project

*(completed by orchestrator: two pi agent runs hit their 20-min cap mid-ticket; engineering below is theirs, push/PR finished centrally)*

## What was implemented (SEC-05)

- `tests/install_command.rs` — 158 lines of red-first fixtures: escaping manifest symlink
  (`../../outside.json`) rejected with byte-identical outside file; symlinked `.ddl` root;
  dangling manifest link; link-swap attempt; legacy migration links still work; Windows
  dir-reparse coverage (cfg-gated).
- `src/dot_ddl.rs` — link-escape rejection (clear error, no outside mutation) + race-free
  no-follow manifest opens via `O_NOFOLLOW` (libc, unix-only): closes the check-then-open
  swap window instead of validating-then-opening.
- `Cargo.toml` — `[target.'cfg(unix)'.dependencies] libc = "0.2"` (design-justified:
  O_NOFOLLOW for TOCTOU-free manifest open, required by the no-race acceptance criterion).
- `openspec/changes/` — proposal defining allowed vs rejected state-root links (legacy
  migration links explicitly supported; broadened rejection specified before enforcement).

## Test results

- `cargo test --locked`: 16/16 test targets ok; only `s2_live_book_sites_return_200`
  fails (pre-existing environmental, fails on base commit — sibling fleet repos absent).
- `cargo fmt --all --check`: clean. `cargo clippy --locked --all-targets -- -D warnings`: clean.

## Commits (branch `feat/ddl-sg8-manifest-escape`, rebased onto latest main)

- `725430d` test(security): red — manifest link escape tests (SEC-05)
- `d0179b8` fix(security): refuse manifest writes redirected by links outside the project
- `5290cd5` test(security): legacy migration link + Windows dir reparse coverage
- `813b1ed` fix(security): race-free no-follow manifest opens close the check-then-open swap window
- `45c1802` docs(spec): OpenSpec proposal — allowed vs rejected state-root links

## Notes / gaps

- First agent run: red+green commits, killed at 20-min cap. Second run: coverage + openspec,
  killed at cap before push/PR. Push/PR finished by orchestrator.
- Windows reparse behavior covered by cfg-gated tests only (no real Windows run available).
