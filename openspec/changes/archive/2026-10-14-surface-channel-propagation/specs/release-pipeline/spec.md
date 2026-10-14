## ADDED Requirements

### Requirement: Channel Propagation Health

The release pipeline SHALL distinguish intentionally disabled distribution
channels from failed channel updates, SHALL retry failed channel updates
idempotently without re-publishing the crate or re-creating the release, and
SHOULD produce a visible incomplete-distribution result for the release when
any channel update fails.

#### Scenario: Failed channel update surfaces

- **WHEN** the Homebrew tap or Scoop bucket update fails after retries
- **THEN** the release run reports an incomplete-distribution result for
  that channel in the job summary
- **AND** the publishing job fails instead of reporting success

#### Scenario: Disabled channel is not a failure

- **WHEN** `TAP_GITHUB_TOKEN` is absent for a release run
- **THEN** the channel update is skipped and reported as `disabled`
- **AND** the release run is not failed by that channel

#### Scenario: Retry is idempotent

- **WHEN** a failed channel update is retried after it had already been
  committed by an earlier attempt
- **THEN** the channel re-render is a byte-identical no-op
- **AND** the GitHub release and crates.io publication are not re-run

#### Scenario: Channel update verifies release inputs

- **WHEN** the declared version, asset tag, or checksum entries do not
  correspond to the selected release
- **THEN** the channel update refuses to write any output and exits
  non-zero before committing
