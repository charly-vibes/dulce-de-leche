# Spec Delta: bootstrap — reject artifacts from an unexpected publisher or build identity

## ADDED Requirements

### Requirement: Release workflow publishes provenance

The release workflow SHALL publish a `provenance.json` manifest binding every
published artifact's sha256 digest to the repository, workflow, and tag that built
the release.

#### Scenario: Provenance manifest published with every release

- **WHEN** the release workflow publishes a release
- **THEN** the release carries `provenance.json` naming `repository`,
  `workflow`, `tag`, and a sha256 digest per published artifact

### Requirement: Build-identity verification before installation

The installer SHALL verify the provenance manifest of a binary release download
against ddl's trust roots (`charly-vibes/dulce-de-leche`, `Release` workflow, and the
tag parsed from the archive URL) and SHALL reject the download before any
destination mutation when identity does not match or the manifest is unavailable.

#### Scenario: Expected identity is accepted

- **WHEN** the provenance manifest names the expected repository, workflow, and the
  release tag, and lists a digest matching the downloaded bytes
- **THEN** the install proceeds and the download is treated as identity-verified

#### Scenario: Altered bytes are rejected

- **WHEN** the downloaded bytes do not match the digest the manifest lists for the
  asset
- **THEN** the download is rejected with `ChecksumMismatch`

#### Scenario: Wrong repository is rejected

- **WHEN** the manifest declares a repository other than
  `charly-vibes/dulce-de-leche`
- **THEN** the download is rejected and the diagnostic names the repository mismatch

#### Scenario: Wrong workflow is rejected

- **WHEN** the manifest declares a workflow other than `Release`
- **THEN** the download is rejected and the diagnostic names the workflow mismatch

#### Scenario: Wrong tag is rejected

- **WHEN** the manifest declares a tag other than the one the asset was fetched from
- **THEN** the download is rejected and the diagnostic names the tag mismatch

#### Scenario: Missing provenance is rejected

- **WHEN** the release publishes no usable provenance manifest (missing, malformed,
  unreadable, or transport-failed)
- **THEN** the download is rejected by default

#### Scenario: Tag-less archive URL is rejected

- **WHEN** the archive URL carries no release tag, so identity cannot be checked
- **THEN** the download is rejected by default

### Requirement: Identity checks are never bypassed

The installer SHALL NOT permit any authorization to bypass a *contradicting*
provenance manifest or a digest mismatch; explicit authorization bridges only the
*absence* of verification.

#### Scenario: Authorization bridges missing provenance only

- **WHEN** the provenance manifest is unavailable AND
  `DDL_ALLOW_UNVERIFIED_RELEASE=1` is set
- **THEN** the install proceeds labeled unverified-authorized

#### Scenario: Authorization does not rescue wrong identity

- **WHEN** the manifest names an unexpected repository, workflow, or tag AND
  `DDL_ALLOW_UNVERIFIED_RELEASE=1` is set
- **THEN** the download is rejected

#### Scenario: Authorization does not rescue altered bytes

- **WHEN** the bytes do not match the manifest's digest AND
  `DDL_ALLOW_UNVERIFIED_RELEASE=1` is set
- **THEN** the download is rejected with `ChecksumMismatch`