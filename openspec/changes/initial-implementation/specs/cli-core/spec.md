## ADDED Requirements
### Requirement: Command Structure
The CLI SHALL provide a flat set of top-level commands, each performing a single orchestration function.

#### Scenario: List available commands
- **WHEN** user runs `ddl --help` or `ddl -h`
- **THEN** the system displays a list of all available commands with brief descriptions
- **AND** shows global flags

#### Scenario: Version display
- **WHEN** user runs `ddl --version` or `ddl -V`
- **THEN** the system displays the ddl version number

#### Scenario: Command not found
- **WHEN** user runs `ddl <unknown-command>`
- **THEN** the system displays a "did you mean" suggestion if a similar command exists
- **AND** exits with a non-zero code

### Requirement: Global Flags
The CLI SHALL support global flags that work with all commands.

#### Scenario: Verbose output
- **WHEN** user passes `-v` or `--verbose`
- **THEN** output includes additional context and installation details

#### Scenario: Quiet mode
- **WHEN** user passes `-q` or `--quiet`
- **THEN** only errors are shown

#### Scenario: Non-interactive mode
- **WHEN** user passes `--yes` or `-y`
- **THEN** the system proceeds with default choices for all confirmations

#### Scenario: JSON output
- **WHEN** user passes `--json`
- **THEN** the system outputs JSON for machine parsing
- **AND** the JSON envelope follows the genesis-vibes envelope format

#### Scenario: NO_COLOR support
- **WHEN** the `NO_COLOR` environment variable is set
- **THEN** the system disables colored output

### Requirement: Exit Codes
The CLI SHALL use consistent exit codes across all commands.

#### Scenario: Success
- **WHEN** a command completes successfully
- **THEN** the system exits with code 0

#### Scenario: Partial failure
- **WHEN** a command partially completes
- **THEN** the system exits with code 1
- **AND** reports which tools succeeded and which failed

#### Scenario: Unrecoverable error
- **WHEN** a command encounters an unrecoverable error
- **THEN** the system exits with code 2
- **AND** displays a diagnostic error with a suggested fix

### Requirement: Help Text
The CLI SHALL provide informative help text for every command.

#### Scenario: Command help
- **WHEN** user runs `ddl <command> --help`
- **THEN** the system displays the command's usage, description, and flags

#### Scenario: Help text includes examples
- **WHEN** user runs `ddl <command> --help`
- **THEN** the help text includes 1-2 usage examples