# Design: update-release-credential-scoping

## Context

SEC-06 baseline: release `contents: write` was workflow-wide, actions used
mutable tags, publish depended only on build jobs, and CI executed an
unchecked bd archive.

## Goals / Non-Goals

- Goal: one-binary release publishing gated on tests + advisory + secret
  checks of the exact tagged commit, with credentials scoped to the publish
  job and supply-chain inputs pinned/verified.
- Non-Goal: changing the install/update protocol, manifest format, or public
  CLI behavior. Non-goal: implementing DDL-wof (publisher/build identity
  rejection) — per the co-mod note it lands in the same release.yml revision
  when that ticket proceeds; this change does not preclude it.
- Non-Goal: enabling GitHub tag rulesets (requires repo admin; documented as
  follow-up after verifying none exist via GitHub API on 2026-10-13).

## Decisions

1. **Structural gating via `needs`, not retries or continue-on-error.**
   GitHub Actions semantics already block a job when any `needs` dependency
   fails, is skipped, or is cancelled — matching the acceptance criterion
   without extra tooling. Regression fixtures assert the `needs` list.
2. **Test/fixtures assert structure, not runtime.** A text-parsing Rust test
   suite (no new dependencies, zero lockfile churn) asserts permissions,
   credential locality, `needs` coverage, SHA pinning, and checksum-before-
   extraction ordering. This runs in `just ci` and cannot drift silently.
3. **Security checks via justfile recipes.** `just advisory` (cargo audit,
   installed with --locked if missing) and `just secret-scan` (git-grep over
   curated high-confidence secret patterns) keep local/CI parity and avoid
   adding third-party actions that would themselves need pinning and review.
   Limitation: the secret scan is pattern-based, not a full gitleaks ruleset.
4. **SHA resolution from upstream.** All action SHAs were resolved from the
   GitHub API against the upstream repos' releases/tags on 2026-10-13 and are
   annotated with the source version in a trailing comment.
5. **bd checksum pinned in-repo.** SHA-256 digest for
   beads_1.3.0_linux_amd64.tar.gz fetched from the release asset digest via
   the GitHub API; verified with `sha256sum -c -` before `tar -xzf`.

## Risks / Trade-offs

- SHA pinning adds maintenance churn on toolchain/action bumps (mitigated by
  version comments on each pin).
- Pattern-based secret scanning has false negatives (accepted; documented).
- cargo audit installs on demand in the release security job (adds ~1 min).
