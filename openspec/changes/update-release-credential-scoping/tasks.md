## Tasks

- [x] Write failing regression fixtures for credential scoping, publish gating, action pinning, and bd archive verification (red)
- [x] Scope contents:write and publication credentials to the publish job in release.yml
- [x] Add test + security jobs on the tagged commit; publish needs them
- [x] Pin all third-party actions to full commit SHAs (release.yml, ci.yml)
- [x] Verify bd tarball checksum before extraction in ci.yml
- [x] Add `just advisory` and `just secret-scan` recipes
- [x] Document credential scopes and verified repo protections
- [x] Fixtures green (5 positive + 1 negative), actionlint clean
