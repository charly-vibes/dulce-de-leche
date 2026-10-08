//! Integration tests for the `ddl init` tool-init cascade (DDL-6zn.3).
//!
//! Semantics under test:
//! - failures are NOT silently swallowed: a failing tool init exits non-zero
//!   and prints a resume summary with the exact finish commands
//! - prerequisite ordering: openspec init runs BEFORE ah init
//! - tools not on PATH but present at the .ddl/bin destination still init
//! - tools without an init command are skipped silently

use assert_cmd::Command;
use std::path::Path;
use std::time::Duration;

/// Timeout for each test — init with --no-install should be instant.
const CMD_TIMEOUT: Duration = Duration::from_secs(10);

/// Create stub executables (name → script body) in `temp/stub-bin` and
/// prepend that dir to the child's PATH. Also stubs `incitaciones` so the
/// skill-install step never hits the npm registry (CI hermeticity).
fn stub_tools(cmd: &mut Command, temp: &tempfile::TempDir, stubs: &[(&str, &str)]) {
    let bin_dir = temp.path().join("stub-bin");
    std::fs::create_dir_all(&bin_dir).unwrap();
    for (name, body) in stubs {
        let stub = bin_dir.join(name);
        std::fs::write(&stub, format!("#!/bin/sh\n{body}\n")).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
    }
    // incitaciones stub: skill install must stay hermetic
    let incit = bin_dir.join("incitaciones");
    if !incit.exists() {
        std::fs::write(&incit, "#!/bin/sh\nexit 0\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&incit, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
    }
    let inherited = std::env::var_os("PATH").unwrap_or_default();
    let mut paths = vec![bin_dir];
    paths.extend(std::env::split_paths(&inherited));
    cmd.env("PATH", std::env::join_paths(paths).unwrap());
}

fn ddl_cmd() -> (Command, tempfile::TempDir) {
    let temp = tempfile::tempdir().unwrap();
    let mut cmd = Command::cargo_bin("ddl").unwrap();
    cmd.current_dir(temp.path());
    (cmd, temp)
}

fn read(temp: &Path, rel: &str) -> String {
    std::fs::read_to_string(temp.join(rel)).unwrap_or_else(|e| panic!("read {rel}: {e}"))
}

#[test]
fn failing_tool_init_fails_run_with_resume_summary() {
    let (mut cmd, temp) = ddl_cmd();
    stub_tools(
        &mut cmd,
        &temp,
        &[("dont", "echo 'no .dont/ project found' >&2\nexit 1")],
    );
    // --human: piped stdout otherwise auto-detects JSON mode, which suppresses
    // the human resume summary (JSON mode puts it on stderr — see next test).
    cmd.args(["init", "--yes", "--no-install", "--human"]);
    cmd.timeout(CMD_TIMEOUT);
    // The old behavior swallowed init failures (exit 0, half-configured repo).
    cmd.assert().failure();
    cmd.assert()
        .stdout(predicates::str::contains("Init cascade failed"))
        .stdout(predicates::str::contains("dont init"))
        .stdout(predicates::str::contains("ddl init --tools dont"));
}

#[test]
fn failing_tool_init_json_mode_puts_summary_on_stderr() {
    let (mut cmd, temp) = ddl_cmd();
    stub_tools(&mut cmd, &temp, &[("dont", "exit 1")]);
    cmd.args(["init", "--yes", "--no-install"]); // piped → JSON envelopes
    cmd.timeout(CMD_TIMEOUT);
    cmd.assert().failure();
    cmd.assert()
        .stderr(predicates::str::contains("finish with: dont init"));
}

#[test]
fn green_cascade_exits_zero() {
    let (mut cmd, temp) = ddl_cmd();
    stub_tools(&mut cmd, &temp, &[("dont", "exit 0")]);
    cmd.args(["init", "--yes", "--no-install"]);
    cmd.timeout(CMD_TIMEOUT);
    cmd.assert().success();
}

#[test]
fn ah_init_runs_after_openspec_init() {
    // ah exits 1 when openspec/ does not exist (real ah behavior).
    // Ordering fix: openspec (creates openspec/) must run first.
    let (mut cmd, temp) = ddl_cmd();
    stub_tools(
        &mut cmd,
        &temp,
        &[
            (
                "ah",
                "[ -d openspec ] || { echo 'openspec/ directory not found' >&2; exit 1; }",
            ),
            ("openspec", "mkdir -p openspec"),
        ],
    );
    cmd.args(["init", "--yes", "--no-install"]);
    cmd.timeout(CMD_TIMEOUT);
    cmd.assert().success();
    // sanity: the stub really created it via the ordered cascade
    assert!(temp.path().join("openspec").is_dir());
}

#[cfg(unix)]
#[test]
fn init_reaches_tool_only_at_ddl_bin_destination() {
    // DDL-6zn.3 vampiro case: downloaded to ~/.ddl/bin, never on PATH.
    // Sandboxed HOME + binary at the .ddl/bin destination; nothing on PATH.
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().join("home");
    let bin = home.join(".ddl").join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let tool = bin.join("dont");
    std::fs::write(&tool, "#!/bin/sh\nexit 0\n").unwrap();
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tool, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    let mut cmd = Command::cargo_bin("ddl").unwrap();
    cmd.current_dir(temp.path());
    stub_tools(&mut cmd, &temp, &[]); // incitaciones stub only
    // Note: `dont` is deliberately NOT in stub-bin here — it lives only at
    // $HOME/.ddl/bin. Inherit PATH but with HOME sandboxed.
    cmd.env("HOME", &home);
    let inherited = std::env::var_os("PATH").unwrap_or_default();
    let mut paths: Vec<_> = std::env::split_paths(&inherited).collect();
    paths.retain(|p| p != &bin); // belt & suspenders: keep bin off PATH
    cmd.env("PATH", std::env::join_paths(&paths).unwrap());
    cmd.args(["init", "--yes", "--no-install", "--tools", "dont"]);
    cmd.timeout(CMD_TIMEOUT);
    cmd.assert().success();
}

#[test]
fn gates_flag_still_works_alongside_cascade() {
    // The .2 acceptance must keep passing with the .3 cascade semantics.
    let (mut cmd, temp) = ddl_cmd();
    stub_tools(&mut cmd, &temp, &[]);
    cmd.args(["init", "--yes", "--no-install", "--gates"]);
    cmd.timeout(CMD_TIMEOUT);
    cmd.assert().success();
    let lefthook = read(temp.path(), "lefthook.yml");
    assert!(lefthook.contains("# ddl:managed:start (pre-commit)"));
    assert!(read(temp.path(), ".beads/config.yaml").contains("no-db: true"));
}
