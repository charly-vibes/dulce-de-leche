## ADDED Requirements

### Requirement: Least-Privilege Release Credentials

The release pipeline SHALL grant `contents: write` and publication credentials
(GITHUB_TOKEN, CARGO_REGISTRY_TOKEN, TAP_GITHUB_TOKEN) only to the publishing
job; all other jobs and the workflow default SHALL be `contents: read`.

#### Scenario: Build job cannot publish

- **WHEN** any build, test, or security job runs
- **THEN** it holds no `contents: write` permission
- **AND** references no publication credential secrets

#### Scenario: Publishing job holds scoped credentials

- **WHEN** the publish job runs
- **THEN** it holds `contents: write` scoped to the repository
- **AND** the crates.io and tap/scoop credentials are referenced only inside it

### Requirement: Publication Gated on Tested Commit

The release pipeline SHALL run the test suite, dependency advisory audit, and
secret scan on the exact tagged commit, and the publishing job SHALL depend on
those jobs so that failed, skipped, or cancelled prerequisites prevent
publication.

#### Scenario: Failing advisory blocks release

- **WHEN** the dependency advisory audit fails for the tagged commit
- **THEN** the publish job never starts
- **AND** no GitHub release, crates.io publish, or tap update occurs

#### Scenario: Skipped prerequisite blocks release

- **WHEN** a test or security job is skipped or cancelled
- **THEN** the publish job does not run

### Requirement: Pinned Third-Party Actions

The release and CI pipelines SHALL pin every third-party action to a full
40-hex commit SHA.

#### Scenario: Mutable tag rejected

- **WHEN** a workflow references an action by tag or branch name
- **THEN** the regression fixture suite fails the change

### Requirement: Verified Bootstrap Archive

The CI pipeline SHALL verify the bd bootstrap archive against a pinned
SHA-256 digest before extraction or execution.

#### Scenario: Tampered archive fails fast

- **WHEN** the downloaded bd tarball does not match the pinned digest
- **THEN** checksum verification fails before any extraction
- **AND** no file from the archive is executed

### Requirement: Separated Build and Publication Authority

The release pipeline design SHALL document credential scopes per job and SHALL
record the actual repository tag protections (not assumed), flagging missing
protections as follow-up recommendations.

#### Scenario: Protection posture documented

- **WHEN** the release workflow is reviewed
- **THEN** the security model header lists each credential's scope
- **AND** the verified tag-protection status and follow-up recommendation
  are recorded
