# Change: Refuse manifest writes redirected by links outside the project

## Why

Security evaluation 2026-10-09, finding SEC-05 (DDL-sg8): a repository
`.ddl/manifest.json` could be a symlink pointing at a file outside the project
(e.g. `../../outside.json`). `ddl install <tool>` then exited 0 and overwrote
that outside file with manifest data. Any tool, script, or person who can plant
a link inside the repository's `.ddl/` tree could redirect ddl's state writes to
arbitrary paths — inside or outside the project.

## What Changes

- **Manifest path links are never followed on write**: every manifest write
  path (`create_at`, `save_manifest`, `record_tool`) opens the manifest with
  `O_NOFOLLOW` (Unix) / `FILE_FLAG_OPEN_REPARSE_POINT` (Windows). A link at the
  manifest path — dangling, in-project, or escaping — fails the open with a
  clear SEC-05 error and nothing is written.
- **The refusal lives inside the open, not only in a pre-check**: the existing
  path-based guard (`ensure_manifest_write_is_local`) still runs first for good
  diagnostics, but check-then-open alone is racy — a link swapped in between the
  two syscalls would still be followed. The no-follow open closes that race;
  the guard cannot be bypassed by timing.
- **Allowed state-root links (explicitly supported)**:
  - `.ddl/` itself may be a link to a directory **inside** the project (e.g.
    `.ddl` -> `.internal-ddl`); writes stay inside the tree.
  - Post-migration legacy links (`.wai` -> `.ddl/wai`, etc. — the Phase 1
    "symlink farm" layout) are untouched by the guard and keep working.
- **Rejected state-root links**:
  - `.ddl/` as a link resolving **outside** the project root — manifest writes
    would land in the linked tree.
  - Any link at the `manifest.json` path itself, regardless of target — it can
    be swapped between check and write, and a write through it lands wherever
    the current target points.
- **Platform behavior**: Unix symlinks and Windows reparse points (symlinks and
  junction-style directory reparse points) are covered by platform-gated tests.

## Impact

- Affected specs: `dot-ddl`
- Affected code: `src/dot_ddl.rs` (guard + no-follow open), `src/lib.rs`,
  `src/main.rs` (error surfacing), `tests/install_command.rs` (regressions)
- Breaking for repositories whose `.ddl/manifest.json` is itself a link — that
  layout was never a documented or supported configuration, and writes through
  it are the vulnerability being closed. In-project `.ddl/` directory links and
  legacy migration links remain supported.
