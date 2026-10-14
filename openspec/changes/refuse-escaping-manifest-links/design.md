# Design: Refuse manifest writes redirected by links outside the project

## Decision: enforce in the open, not in a pre-check

### Context

The naive fix — check `symlink_metadata(manifest)` before opening — is
check-then-open. Between the two syscalls an attacker can swap the manifest for
a link; the subsequent `open()` follows it and the write lands at the target.
The ticket requires the link-swap case to be covered "without a check-then-open
race".

### Options considered

1. **No-follow open (chosen)** — open the manifest with `O_NOFOLLOW` on Unix
   and `FILE_FLAG_OPEN_REPARSE_POINT` on Windows. The kernel refuses a link at
   the final path component as part of the open itself; there is no window
   between check and open to exploit. Dangling links are refused too
   (`O_NOFOLLOW` + `O_CREAT` fails with ELOOP on a trailing symlink).
2. Path-based pre-check only — rejected: racy by construction.
3. Open, then compare `fstat(fd)` against `lstat(path)` dev/ino — rejected: a
   hardlink planted inside `.ddl/` pointing to an outside file defeats the
   inode comparison, and the extra logic is harder to audit than a kernel-level
   no-follow flag.
4. `openat` with a pinned directory fd — strictly stronger (also hardens the
   directory components), but a larger protocol change across every write site.
   Deferred; documented as a future hardening step.

### Rationale

The no-follow flag moves enforcement into the syscall that performs the write
path's first mutation. Path-based checks remain only as a diagnostic layer that
produces better error messages (naming the `.ddl/` directory link and the
resolved outside root) before the open backs them up.

## Allowed vs. rejected state-root links

| Link location        | Target inside project            | Target outside project |
| -------------------- | -------------------------------- | ---------------------- |
| `.ddl/` (directory)  | **allowed** (e.g. symlink farm)  | **rejected** (SEC-05)  |
| `manifest.json`      | **rejected** (swappable)         | **rejected** (SEC-05)  |
| `.ddl/<tool>/` links | **allowed** (legacy migration)   | **rejected**           |

Manifest-path links are rejected even in-project because a link is mutable
state: any check result describes the link, not the write destination.

## Platform notes

- Unix: `O_NOFOLLOW` via `libc` (values differ per Unix, so the named constant
  is required; `libc` is already in the dependency graph, so lockfile churn is
  one edge).
- Windows: `FILE_FLAG_OPEN_REPARSE_POINT` opens the reparse point itself
  instead of following it. Directory reparse points (junctions/symlink dirs) at
  `.ddl/` are covered by the canonicalize guard plus platform tests. Creating
  file/dir symlinks on Windows requires admin/dev-mode; the tests skip
  gracefully when the privilege is missing.

## Out of scope (residual)

- Hardlinks inside `.ddl/` pointing to outside files: cannot be committed via
  git and are not creatable by a repo snapshot alone; note as a known residual
  vector for local attackers with `.ddl/` write access.
- `openat`/dirfd-based full path hardening (option 4).
