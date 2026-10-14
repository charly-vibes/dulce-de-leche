# Tasks: Refuse manifest writes redirected by links outside the project

## 1. Red (regression fixtures first)

- [x] 1.1 `tests/install_command.rs`: manifest symlink -> outside file refused,
      outside file byte-for-byte unchanged
- [x] 1.2 `tests/install_command.rs`: dangling manifest symlink refused, target
      not created
- [x] 1.3 `tests/install_command.rs`: `.ddl/` directory link outside the
      project refused
- [x] 1.4 `tests/install_command.rs`: in-project `.ddl/` directory link allowed
- [x] 1.5 `tests/install_command.rs` (windows): reparse-point manifest refused

## 2. Green (guard + race-free enforcement)

- [x] 2.1 `src/dot_ddl.rs`: `ensure_manifest_write_is_local` guard (symlink
      manifest + `.ddl/` outside-root check)
- [x] 2.2 `src/dot_ddl.rs`: `open_manifest_no_follow` — `O_NOFOLLOW` (Unix) /
      `FILE_FLAG_OPEN_REPARSE_POINT` (Windows) manifest opens
- [x] 2.3 All manifest write sites (`create_at`, `save_manifest`,
      `record_tool`) open through the no-follow helper

## 3. Coverage completion (this change)

- [x] 3.1 Unit tests: link-swap case — open itself refuses after the manifest
      is swapped to a link, with no prior check involved
- [x] 3.2 Unit test: dangling link refused by the no-follow open, target not
      created
- [x] 3.3 Unit test: in-project manifest link also refused (links are mutable
      state)
- [x] 3.4 Integration test: legacy migration links (`.wai` -> `.ddl/wai`)
      keep working end-to-end
- [x] 3.5 Integration tests (windows): `.ddl/` directory reparse point outside
      the project refused; inside the project allowed

## 4. Verification

- [x] 4.1 `cargo test --locked` green (except pre-existing environmental
      `s2_live` failure, failing on base commit too)
- [x] 4.2 `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings`
