//! Integration tests for doctor subprocess handling (health spec):
//! tools without a doctor subcommand degrade to basic checks, and doctor
//! keeps running the remaining tools after a tool's subprocess fails.

mod common;

use assert_cmd::Command;
use std::time::Duration;

/// Timeout for each test.
const CMD_TIMEOUT: Duration = Duration::from_secs(90);

#[test]
fn test_doctor_tool_subprocess_fail_does_not_abort_fleet() {
    // A managed tool whose doctor subprocess fails (fake binary exiting 1
    // for every subcommand) must not abort the doctor run: the remaining
    // tools are still checked and a diagnostic for the failing tool is
    // included (health/tool-subprocess-fails, tool-doctor-not-available).
    let temp = tempfile::tempdir().unwrap();
    let fake_bin = temp.path().join("fakebin");
    std::fs::create_dir_all(&fake_bin).unwrap();
    for name in [
        "wai",
        "dont",
        "ah",
        "pretender",
        "testaruda",
        "vampiro",
        "turu",
    ] {
        let fake = fake_bin.join(name);
        std::fs::write(&fake, "#!/usr/bin/env bash\nexit 1\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
    }

    let mut cmd = Command::cargo_bin("ddl").unwrap();
    common::clean_git_env(&mut cmd);
    // The fake bin dir is FIRST on PATH so the fakes shadow real tools.
    cmd.env(
        "PATH",
        format!(
            "{}:{}",
            fake_bin.display(),
            std::env::var("PATH").unwrap_or_default()
        ),
    );
    cmd.current_dir(temp.path());
    cmd.arg("--human").arg("doctor");
    cmd.timeout(CMD_TIMEOUT);

    let assertion = cmd.assert().success();
    let output = assertion.get_output().stdout.clone();
    let text = String::from_utf8(output).unwrap();
    assert!(
        !text.is_empty(),
        "doctor must still produce a report when tool subprocesses fail"
    );
    // The report must mention at least one checked tool.
    assert!(
        text.contains("wai") || text.contains("dont"),
        "doctor report must include per-tool entries, got:\n{text}"
    );
}
