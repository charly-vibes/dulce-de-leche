# CLI Core

## Purpose

Define the core command structure, global flags, and output conventions for the `ddl` CLI. All dulce-de-leche commands follow these patterns.

## Problem Statement

For `ddl` to be a reliable bootstrap and orchestration tool, it requires a stable and predictable command interface. Without a consistent command structure, users would face a steep learning curve and inconsistent interactions when managing the charly-vibes toolset.

## Design Rationale

### Command Structure: Single-Level Nouns

ddl uses a flat noun-based command structure (e.g., `ddl init`, `ddl status`, `ddl upgrade`) rather than a verb-noun hierarchy. This is intentional: ddl is a thin orchestrator with a small number of commands that each do one thing well. The flat structure keeps the CLI discoverable and fast.

### Output Format

All commands output human-readable text by default. Commands that return structured data support `--json` for machine parsing. Error output follows the self-healing pattern: describe the problem, suggest a fix, and provide the exact command to run.

## Scope

### Non-Goals

- Subcommand nesting (no `ddl tool install wai` — use `ddl install wai`)
- Interactive mode beyond confirmation prompts (use `--yes` for non-interactive)
- Pager integration (output is designed for terminal and CI)

## Requirements

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
- **AND** the JSON envelope follows the genesis-vibes envelope format: `{"ok": true, "data": {...}, "warnings": [], "hints": []}`

#### Scenario: NO_COLOR support

- **WHEN** the `NO_COLOR` environment variable is set
- **THEN** the system disables colored output

### Requirement: Exit Codes

The CLI SHALL use consistent exit codes across all commands.

#### Scenario: Success

- **WHEN** a command completes successfully
- **THEN** the system exits with code 0

#### Scenario: Partial failure

- **WHEN** a command partially completes (e.g., some tools installed, some failed)
- **THEN** the system exits with code 1
- **AND** reports which tools succeeded and which failed

#### Scenario: Unrecoverable error

- **WHEN** a command encounters an unrecoverable error (e.g., invalid arguments, missing platform)
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