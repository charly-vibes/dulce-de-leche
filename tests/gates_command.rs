//! Integration tests for `ddl init --gates`.
//!
//! Acceptance (DDL-6zn.2): `ddl init -y --gates` in a fresh repo yields
//! lefthook hard gates (managed blocks INSIDE `commands:`), tool-data
//! .gitignore entries, beads no-db stamping — idempotent on re-run.

use assert_cmd::Command;
use std::path::Path;
use std::time::Duration;

/// Timeout for each test — init with --no-install should be instant.
const CMD_TIMEOUT: Duration = Duration::from_secs(10);

/// Prepend a stub `incitaciones` executable to the child's PATH so init's
/// skill-install step runs the stub instead of a real `npx --yes` network
/// install (CI hermeticity — see tests/init_noninteractive.rs).
fn stub_incitaciones_on_path(cmd: &mut Command, temp: &tempfile::TempDir) {
    let bin_dir = temp.path().join("stub-bin");
    std::fs::create_dir_all(&bin_dir).unwrap();
    let stub = bin_dir.join("incitaciones");
    std::fs::write(&stub, "#!/bin/sh\nexit 0\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).unwrap();
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
    stub_incitaciones_on_path(&mut cmd, &temp);
    (cmd, temp)
}

fn read(temp: &Path, rel: &str) -> String {
    std::fs::read_to_string(temp.join(rel)).unwrap_or_else(|e| panic!("read {rel}: {e}"))
}

#[test]
fn help_advertises_gates_flag() {
    let (mut cmd, _temp) = ddl_cmd();
    cmd.arg("init").arg("--help");
    cmd.timeout(CMD_TIMEOUT);
    cmd.assert()
        .success()
        .stdout(predicates::str::contains("--gates"));
}

#[test]
fn gates_wires_lefthook_gitignore_and_beads_in_fresh_repo() {
    let (mut cmd, temp) = ddl_cmd();
    cmd.args(["init", "--yes", "--no-install", "--gates"]);
    cmd.timeout(CMD_TIMEOUT);
    cmd.assert().success();

    // lefthook.yml: both hooks, managed blocks inside commands:
    let lefthook = read(temp.path(), "lefthook.yml");
    assert!(lefthook.contains("# ddl:managed:start (pre-commit)"));
    assert!(lefthook.contains("# ddl:managed:start (pre-push)"));
    assert!(lefthook.contains("run: ah check"));
    assert!(lefthook.contains("run: pretender check --mode gate"));
    assert!(lefthook.contains("run: spk lint openspec"));
    for line in lefthook.lines() {
        if line.contains("ddl-ah-check") || line.contains("ddl-pretender-gate") {
            assert!(
                line.starts_with("    "),
                "managed entry not inside commands:: {line:?}"
            );
        }
    }

    // .gitignore: tool data dirs
    let gitignore = read(temp.path(), ".gitignore");
    assert!(gitignore.contains(".testaruda/"));
    assert!(gitignore.contains(".pretender/"));

    // beads: no-db stamped
    let beads = read(temp.path(), ".beads/config.yaml");
    assert!(beads.contains("no-db: true"));
}

#[test]
fn gates_is_idempotent_on_rerun() {
    let (mut cmd, temp) = ddl_cmd();
    cmd.args(["init", "--yes", "--no-install", "--gates"]);
    cmd.timeout(CMD_TIMEOUT);
    cmd.assert().success();

    let lefthook_before = read(temp.path(), "lefthook.yml");
    let gitignore_before = read(temp.path(), ".gitignore");

    let (mut cmd2, _temp2) = ddl_cmd();
    cmd2.current_dir(temp.path());
    stub_incitaciones_on_path(&mut cmd2, &temp);
    cmd2.args(["init", "--yes", "--no-install", "--gates"]);
    cmd2.timeout(CMD_TIMEOUT);
    cmd2.assert().success();

    assert_eq!(
        read(temp.path(), "lefthook.yml"),
        lefthook_before,
        "lefthook.yml must be byte-identical on re-run (no duplicate blocks)"
    );
    assert_eq!(
        read(temp.path(), ".gitignore"),
        gitignore_before,
        ".gitignore must not gain duplicate entries on re-run"
    );
    assert_eq!(
        lefthook_before
            .matches("# ddl:managed:start (pre-commit)")
            .count(),
        1
    );
}
