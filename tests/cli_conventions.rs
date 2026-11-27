//! Integration tests for cross-cutting CLI conventions (cli-core spec):
//! unknown-command handling, help text (usage/description/examples),
//! `ddl --help` command listing, NO_COLOR, and quiet mode.

mod common;

use assert_cmd::Command;
use predicates::prelude::*;
use std::time::Duration;

/// Timeout for each test.
const CMD_TIMEOUT: Duration = Duration::from_secs(60);

/// All subcommands whose `--help` must carry usage examples (cli-core).
const ALL_COMMANDS: &[&str] = &[
    "init",
    "install",
    "catalog",
    "feedback",
    "status",
    "doctor",
    "version",
    "upgrade",
    "migrate",
    "scope",
    "setup",
    "completions",
];

fn ddl_cmd() -> Command {
    let mut cmd = Command::cargo_bin("ddl").unwrap();
    common::clean_git_env(&mut cmd);
    cmd.timeout(CMD_TIMEOUT);
    cmd
}

#[test]
fn test_cli_unknown_command_fails_with_did_you_mean() {
    // A misspelled subcommand must fail (exit 2) and offer the closest
    // real command (clap suggestion).
    ddl_cmd()
        .args(["docotor"])
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("did you mean") .or(predicate::str::contains("similar subcommand exists")));
}

#[test]
fn test_cli_unknown_command_without_close_match_still_fails() {
    ddl_cmd()
        .args(["frobnicate-xyz"])
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("unrecognized subcommand"));
}

#[test]
fn test_cli_help_lists_every_command_with_descriptions() {
    let output = ddl_cmd()
        .args(["--help"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let help = String::from_utf8(output).unwrap();
    for command in ALL_COMMANDS {
        assert!(
            help.contains(&format!("  {command}")),
            "`ddl --help` must list command `{command}`"
        );
    }
    // Global flags shown.
    assert!(help.contains("--verbose"), "--help must show global flags");
    assert!(help.contains("--json"), "--help must show global flags");
    assert!(help.contains("--version"), "--help must show global flags");
}

#[test]
fn test_cli_every_command_help_shows_usage_and_examples() {
    for command in ALL_COMMANDS {
        let output = ddl_cmd()
            .args([command, "--help"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let help = String::from_utf8(output).unwrap();
        assert!(
            help.contains("Usage:"),
            "`ddl {command} --help` must show usage"
        );
        assert!(
            help.contains("Examples:"),
            "`ddl {command} --help` must include usage examples, got:\n{help}"
        );
    }
}

#[test]
fn test_cli_no_color_disables_ansi_output() {
    // NO_COLOR must never produce ANSI escapes (cli-core/no-color-support).
    // Drift note (DDL-oymq): ddl currently emits no ANSI color at all —
    // this pins that NO_COLOR output is plain text either way.
    let output = ddl_cmd()
        .env("NO_COLOR", "1")
        .args(["--human", "status"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(output).unwrap();
    assert!(
        !text.contains('\x1b'),
        "NO_COLOR output must not contain ANSI escapes"
    );
}

#[test]
fn test_cli_quiet_suppresses_non_error_output() {
    // `-q` suppresses non-error output: the version listing is not printed,
    // while the command still exits successfully.
    let output = ddl_cmd()
        .args(["-q", "--human", "version"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(output).unwrap().trim().to_string();
    assert!(
        text.is_empty(),
        "`ddl -q version` must print nothing (only errors are shown), got: {text:?}"
    );
}
