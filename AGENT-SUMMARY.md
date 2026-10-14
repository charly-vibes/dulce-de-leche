# AGENT-SUMMARY — DDL-hsn: networked-mock e2e test for checksum-mismatch abort

## What was implemented

- **`tests/install_policy.rs`** — new self-contained `checksum_mismatch_e2e`
  module (clearly separated from the unit-policy fixtures per the CO-MOD
  note; no shared state):
  - a std-only HTTP/1.1 mock (TcpListener, no new dependencies) serving a
    GitHub-release surface: latest-release lookup, release asset,
    `checksums.txt`, `provenance.json`; every served request path is
    recorded so tests can prove the client was actually routed to the mock;
  - `mismatched_download_aborts_without_installing` — drives the REAL
    `ddl install wai` binary (`CARGO_BIN_EXE_ddl`) in a hermetic sandbox
    (temp HOME → isolated `~/.ddl/bin` destination, temp CWD → isolated
    `.ddl` manifest, stripped PATH → no preinstalled managed tools and
    structurally zero cargo, proxy vars removed). checksums.txt lists a
    well-formed 64-hex digest that matches nothing. Asserts the Must
    criteria: **nonzero exit**, error reported as a checksum rejection,
    **destination untouched**, **no cargo fallback** (`no "cargo install"`
    in output), and the mock served the full lookup chain.
  - `matching_download_installs_verified` — the valid counterpart: honest
    digest + DDL-wof provenance manifest built from the lib's trust-root
    constants (`EXPECTED_PROVENANCE_REPOSITORY`/`_WORKFLOW`, not
    re-hardcoded). Asserts success, "sha256 verified" status, extracted
    binary content at the destination, and that checksums.txt +
    provenance.json were consulted.
- **`src/installer.rs`** — narrow test-only seam:
  `test_release_base_url()` honors `DDL_TEST_RELEASE_BASE_URL` **only**
  when (a) the binary was built with debug assertions (`cargo test`/dev
  build) and (b) the value targets loopback (`http://127.0.0.1`,
  `http://localhost`, `http://[::1]`). Release builds always talk to
  api.github.com/github.com — supply-chain safe (SEC-03). Documented with
  Purpose/Responsibilities/Rationale. `install_binary`'s release-lookup
  and download URLs route through `release_api_base()` /
  `release_download_base()`; the mock URLs keep the
  `releases/download/<tag>/` shape so DDL-wof tag extraction is exercised
  unchanged.

## TDD evidence (red → green)

- **Red** (`4046245`): both e2e tests fail — the child ignored the env var,
  talked to real GitHub, and the mock received **zero** requests; full
  suite otherwise green (fmt/clippy pass, all pre-existing tests pass).
- **Green** (`770227c`): with the seam, both tests pass. Stable across 3
  consecutive re-runs.
- No existing assertions weakened; existing checksum/provenance unit
  coverage intact (22/22 in install_policy.rs).

## Test results

- `cargo test --locked --test install_policy`: **22 passed, 0 failed**
  (20 pre-existing + 2 new).
- `cargo test --locked` full suite: all green **except** one pre-existing,
  environmental failure — `standard_conformance::s2_live_book_sites_return_200`
  ("no Pages-deploying repos detected"): it scans sibling repo checkouts on
  disk and fails identically with my diff stashed (verified on clean HEAD).
  Unrelated to this change.
- `cargo fmt --all` and `cargo clippy --all-targets -- -D warnings`: clean.

## Commits

- `4046245` — test(ddl-hsn): add networked-mock e2e regression for
  checksum-mismatch abort (red)
- `770227c` — fix(ddl-hsn): debug-only loopback release-endpoint seam makes
  e2e checksum tests green

## PR

https://github.com/charly-vibes/dulce-de-leche/pull/72 (ready for review,
not merged — per protocol).

## Notes / decisions

- Zero dependency changes; `Cargo.lock` untouched.
- Provenance fixture initially declared the tool's own repo
  (`charly-vibes/wai`) and was rejected by DDL-wof's trust root; fixed by
  importing the lib constants — the discriminator between the scenarios is
  now strictly the digest, as the ticket intends.
- **Hook carve-out (noted per constraint 5):** the pre-push `gates` hook
  (`just pre-push`) failed environmentally, not on this diff: the hook env
  exports `GIT_DIR` (main checkout's `.git`), and `doctor_command` tests
  spawn `git init` children that inherit it → "could not lock config file
  …/.git/config". The same tests pass standalone in this worktree (verified
  directly, twice). fmt/clippy/test were run manually before every commit.
  Pushes therefore used `git push --no-verify`; commits used the normal
  path (pre-commit hooks passed).
- No openspec change proposal drafted: no install/manifest-protocol
  behavior change ships to release builds (the seam is debug-build-only),
  so no spec delta is warranted; the test itself is the deliverable.
