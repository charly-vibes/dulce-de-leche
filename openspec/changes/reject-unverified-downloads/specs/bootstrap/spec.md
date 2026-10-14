# Change: Reject release downloads when required verification is unavailable

## ADDED Requirements

### Requirement: Release download verification availability

The installer SHALL treat sha256 verification of a binary release download as a
precondition: the download is installed only when the checksum metadata is available,
well-formed, and lists the asset, and the downloaded bytes match the published digest.

#### Scenario: Missing checksum metadata

- **WHEN** a release publishes no checksum source for the downloaded asset
- **THEN** the download is rejected with `ChecksumUnavailable` and a nonzero exit
- **AND** no bytes are written to the install destination

#### Scenario: Malformed checksum metadata

- **WHEN** the release's checksums.txt (or per-asset .sha256) contains no valid
  64-hex-digit digest
- **THEN** the download is rejected with `ChecksumUnavailable`

#### Scenario: Incomplete metadata (asset not listed)

- **WHEN** the release publishes a checksums.txt that does not list the downloaded
  asset
- **THEN** the download is rejected with `ChecksumUnavailable`

#### Scenario: Checksum metadata transport failure

- **WHEN** the checksum metadata request fails (network error)
- **THEN** the download is rejected with `ChecksumUnavailable`
- **AND** the rejection is indistinguishable-in-severity from the missing case
  (transport failure must not degrade to skip-and-install)

#### Scenario: Published checksum mismatch never falls back to cargo

- **WHEN** the published digest does not match the downloaded bytes
- **THEN** the install fails with `ChecksumMismatch`
- **AND** the cargo fallback is not attempted (only `NoReleaseBinary` triggers it)
- **AND** `DDL_ALLOW_UNVERIFIED_RELEASE=1` does not bypass the mismatch

### Requirement: Explicit legacy-unverified authorization

The installer SHALL permit an unverified release download only when the user
explicitly sets `DDL_ALLOW_UNVERIFIED_RELEASE=1` (or `true`/`yes`), and SHALL label
the install as unverified-authorized in human and JSON output.

#### Scenario: Authorized unverified install

- **WHEN** checksum metadata is unavailable AND
  `DDL_ALLOW_UNVERIFIED_RELEASE=1` is set
- **THEN** the install proceeds
- **AND** the install message carries
  `(unverified — explicitly authorized via DDL_ALLOW_UNVERIFIED_RELEASE)`
- **AND** JSON events record `"verification": "unverified-authorized"`

#### Scenario: Verified install

- **WHEN** the published digest matches the downloaded bytes
- **THEN** the install message carries `(sha256 verified)`
- **AND** JSON events record `"verification": "verified"`

## Implementation tasks

- [x] Failing policy fixtures in `tests/install_policy.rs` (red)
- [x] `ChecksumLookup` + `enforce_checksum_policy` pure policy (green)
- [x] `DdlError::ChecksumUnavailable` diagnostic
- [x] Output distinction (`VerificationStatus::suffix`, JSON `verification` field)
- [ ] Follow-up tickets (not this slice): publisher/build-identity checks (DDL-wof),
      replay from exact artifact identities (DDL-5bs)
