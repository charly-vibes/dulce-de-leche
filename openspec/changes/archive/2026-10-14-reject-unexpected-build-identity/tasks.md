# Tasks: reject-unexpected-build-identity

- [x] tests/install_policy.rs — failing DDL-wof fixtures (red): altered bytes, wrong
  repository, wrong workflow, wrong tag, missing provenance, plus
  acceptance/authorization fixtures
- [x] src/installer.rs — `enforce_provenance_policy`, `ProvenanceManifest`,
  `ProvenanceLookup`, trust-root constants, tag parsing, networked
  `fetch_provenance`; chained into `verify_download` after the checksum policy
- [x] .github/workflows/release.yml — generate and publish `provenance.json` for
  every supported binary
- [x] Trust roots, residual limitation, and legacy transition documented
  (installer.rs module docs + design.md)
- [x] Verifier-dependency design justification (design.md)
- [ ] Follow-up (out of scope, needs its own design pass): sigstore
  `actions/attest-build-provenance` + sigstore verifier, per design.md justification