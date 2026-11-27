//! Child-policy, probing, init, and method tests.

use crate::installer::*;
use std::path::Path;

#[cfg(test)]
// ===== DDL-uu8 (SEC-04): restricted-privilege policy =====
#[test]
fn scope_authorization_matrix() {
    // GlobalSkill requires explicit DDL_ALLOW_GLOBAL_SKILL=1.
    assert!(scope_authorize_with(ExecutionScope::GlobalSkill, |_| None).is_err());
    assert!(
        scope_authorize_with(ExecutionScope::GlobalSkill, |v| {
            if v == ALLOW_GLOBAL_SKILL_ENV {
                Some("1".into())
            } else {
                None
            }
        })
        .is_ok()
    );
    // PackageBuild and ToolInit are authorized by the explicit
    // user-invoked command itself (no extra grant).
    assert!(scope_authorize_with(ExecutionScope::PackageBuild, |_| None).is_ok());
    assert!(scope_authorize_with(ExecutionScope::ToolInit, |_| None).is_ok());
}

#[test]
fn scope_rejection_message_names_the_authorization_variable() {
    let err = scope_authorize_with(ExecutionScope::GlobalSkill, |_| None).unwrap_err();
    assert!(err.to_string().contains(ALLOW_GLOBAL_SKILL_ENV));
}

#[test]
fn write_roots_reject_out_of_scope_paths() {
    let temp = tempfile::tempdir().unwrap();
    let project = temp.path().join("project");
    std::fs::create_dir_all(&project).unwrap();
    // Inside the project root: approved for tool-init.
    assert!(
        check_write_root(
            ExecutionScope::ToolInit,
            &project.join("openspec"),
            Some(&project)
        )
        .is_ok()
    );
    // Outside every approved root: rejected.
    assert!(
        check_write_root(
            ExecutionScope::ToolInit,
            Path::new("/etc/passwd"),
            Some(&project)
        )
        .is_err()
    );
    // The global-skill scope's declared target is ~/.agents/skills.
    if let Some(skills_dir) = crate::skills::global_skills_dir() {
        assert!(
            check_write_root(
                ExecutionScope::GlobalSkill,
                &skills_dir.join("commit"),
                Some(&project)
            )
            .is_ok()
        );
        assert!(
            check_write_root(
                ExecutionScope::GlobalSkill,
                Path::new("/etc/passwd"),
                Some(&project)
            )
            .is_err()
        );
    }
}

#[test]
fn child_env_allowlist_excludes_credentials() {
    assert!(CHILD_ENV_ALLOWLIST.contains(&"PATH"));
    assert!(CHILD_ENV_ALLOWLIST.contains(&"HOME"));
    assert!(!CHILD_ENV_ALLOWLIST.contains(&"DDL_TEST_CANARY_CREDENTIAL"));
    assert!(!CHILD_ENV_ALLOWLIST.contains(&"GITHUB_TOKEN"));
}

#[test]
fn child_policy_platform_matrix_is_supported() {
    // The env-allowlist + authorization mechanism is portable across the
    // platforms ddl supports; anything else must fail closed.
    assert!(child_policy_supported());
}

#[test]
fn test_parse_version_line_variants() {
    assert_eq!(parse_version_line("ddl", "ddl 0.6.0"), Some("0.6.0".into()));
    assert_eq!(
        parse_version_line("bd", "bd version 1.3.0"),
        Some("1.3.0".into())
    );
    assert_eq!(parse_version_line("wai", "v1.2.3"), Some("1.2.3".into()));
    assert_eq!(
        parse_version_line("x", "single-token"),
        Some("single-token".into())
    );
}

// Unix-only: the stub is a shebang script; on Windows the probe would
// need an .exe stub, which the executor shims differently.
#[cfg(unix)]
#[test]
fn test_probe_version_at_finds_version_in_ddl_bin() {
    // DDL-x0m: after a binary download to .ddl/bin (not yet on PATH),
    // the probe must run the binary at its destination directly.
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("fake-tool");
    std::fs::write(&script, "#!/bin/sh\necho \"fake-tool version 9.9.9\"\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    assert_eq!(
        probe_version_at(&script, "fake-tool"),
        Some("9.9.9".to_string())
    );
    assert_eq!(
        probe_version_at(dir.path().join("nope").as_path(), "fake-tool"),
        None
    );
}

// --- DDL-6zn.3: init-cascade semantics ---------------------------------

#[test]
fn init_command_wai_ah_and_testaruda_run_init() {
    // ah-init/wai-init/testaruda-init contracts (bootstrap spec): the
    // installed tool's own `init` runs right after installation.
    assert_eq!(init_command("wai", false), Some(("wai", vec!["init"])));
    assert_eq!(init_command("ah", false), Some(("ah", vec!["init"])));
    assert_eq!(
        init_command("testaruda", false),
        Some(("testaruda", vec!["init"]))
    );
}

#[test]
fn init_command_per_tool_and_mode() {
    // dont runs `init` (creates project state) — the old `prime --plain`
    // failed circularly: no .dont/ project exists until dont init runs.
    assert_eq!(init_command("dont", false), Some(("dont", vec!["init"])));
    // pretender's wizard prompt leaked into -y output — non-interactive
    // mode must pass --non-interactive (best-guess defaults).
    assert_eq!(
        init_command("pretender", true),
        Some(("pretender", vec!["init", "--non-interactive"]))
    );
    assert_eq!(
        init_command("pretender", false),
        Some(("pretender", vec!["init"]))
    );
    // openspec: skip the AI-tool config prompt non-interactively.
    assert_eq!(
        init_command("openspec", true),
        Some(("openspec", vec!["init", "--tools", "none"]))
    );
    // tools without an init command are skipped silently (fotos-mcp's
    // 'expect initialize request' failure class).
    assert_eq!(init_command("fotos-mcp", false), None);
    assert_eq!(init_command("turu", true), None);
    assert_eq!(init_command("incitaciones", false), None);
    assert_eq!(init_command("unknown-tool", false), None);
}

#[test]
fn prepend_path_puts_ddl_bin_first() {
    use std::ffi::OsString;
    let bin = std::path::PathBuf::from("/home/t/.ddl/bin");
    let expected: OsString = std::env::join_paths(["/home/t/.ddl/bin", "/usr/bin"]).unwrap();
    assert_eq!(
        prepend_path(&bin, Some(&OsString::from("/usr/bin"))),
        expected
    );
    assert_eq!(prepend_path(&bin, None), OsString::from("/home/t/.ddl/bin"));
    assert_eq!(
        prepend_path(&bin, Some(&OsString::new())),
        OsString::from("/home/t/.ddl/bin")
    );
}

#[test]
fn resume_summary_lists_exact_finish_commands() {
    let failures = vec![
        InitFailure {
            tool: "dont".into(),
            detail: "exited with code 1".into(),
        },
        InitFailure {
            tool: "ah".into(),
            detail: "openspec/ directory not found".into(),
        },
    ];
    let summary = resume_summary(&failures).join("\n");
    assert!(
        summary.contains("dont init"),
        "missing tool's own init cmd: {summary}"
    );
    assert!(
        summary.contains("ddl init --tools dont"),
        "missing re-run cmd: {summary}"
    );
    assert!(summary.contains("ah init"));
    assert!(summary.contains("openspec/ directory not found"));
}

#[test]
fn test_binary_dest_dir_pure() {
    use crate::platform::Os;
    let home = std::path::PathBuf::from("/home/tester");
    assert_eq!(
        binary_dest_dir(&Os::Linux, Some(home.as_path()), None),
        std::path::PathBuf::from("/home/tester/.ddl/bin")
    );
    assert_eq!(
        binary_dest_dir(&Os::Macos, Some(home.as_path()), None),
        std::path::PathBuf::from("/home/tester/.ddl/bin")
    );
    let lad = std::path::PathBuf::from("C:\\Users\\u\\AppData\\Local");
    assert_eq!(
        binary_dest_dir(&Os::Windows, None, Some(lad.as_path())),
        lad.join("ddl").join("bin")
    );
}

#[cfg(unix)]
#[test]
fn test_resolve_version_falls_back_to_dest_when_not_on_path() {
    use std::os::unix::fs::PermissionsExt;
    let temp = tempfile::tempdir().unwrap();
    let dest_dir = temp.path().join(".ddl").join("bin");
    std::fs::create_dir_all(&dest_dir).unwrap();
    let script = dest_dir.join("faketool");
    std::fs::write(&script, "#!/bin/sh\necho \"faketool version 1.2.3\"\n").unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

    // The binary is not on PATH — the dest fallback must still find it.
    assert_eq!(
        resolve_with_dest("faketool", Some(&dest_dir)),
        Some("1.2.3".to_string())
    );
}

#[test]
fn test_resolve_with_dest_none_when_nothing_installed() {
    // A name that exists nowhere on PATH and in a dest dir without it.
    let empty = tempfile::tempdir().unwrap();
    assert_eq!(
        resolve_with_dest("definitely-not-real-xyz", Some(empty.path())),
        None
    );
}

/// Verify verbose_print exists and accepts both values without panicking.
/// The actual stderr output is verified by integration tests that run
/// the ddl binary with --verbose.
#[test]
fn test_verbose_print_accepts_bool() {
    // Must not panic for either value
    verbose_print(true, "trace message");
    verbose_print(false, "trace message");
}

#[test]
fn test_fallback_after_binary_failure_is_cargo_only() {
    // bootstrap/binary-download-returns-404 + -without-cargo +
    // no-release-binary-and-no-cargo: cargo is the ONLY fallback (DDL-ei3);
    // without cargo there is no further install method (None → the caller
    // fails naming both remedies).
    use crate::installer::method::{fallback_after_binary_failure, InstallMethod};
    assert_eq!(
        fallback_after_binary_failure(true),
        Some(InstallMethod::Cargo)
    );
    assert_eq!(fallback_after_binary_failure(false), None);
}

#[test]
fn test_check_prerequisites_pass_for_available_methods() {
    use crate::installer::method::{best_install_method, InstallMethod};
    // Standard environments (dev sandbox, CI ubuntu runners) ship curl and
    // cargo; the methods' prerequisite gates must accept them.
    InstallMethod::Binary
        .check_prerequisites()
        .expect("binary download prerequisites (curl or wget) must be met");
    InstallMethod::Cargo
        .check_prerequisites()
        .expect("cargo prerequisites must be met");
    InstallMethod::Skipped.check_prerequisites().expect("skipped is always ok");
    // npm tools (incitaciones) always take the npm method.
    let npm_tool = crate::platform::MANAGED_TOOLS
        .iter()
        .find(|t| t.npm_package.is_some())
        .expect("registry must contain an npm-distributed tool");
    assert_eq!(
        best_install_method(npm_tool, &crate::platform::Platform::detect().unwrap()),
        InstallMethod::Npm
    );
}

#[test]
fn test_upgrade_decision_policy_covers_up_to_date_and_rejections() {
    // version-management/upgrade-*: the shared release-age policy decides
    // UpToDate / Deferred (cool-down) / Upgrade deterministically.
    use crate::installer::method::{upgrade_decision, ReleaseCandidate, UpgradeDecision};
    use crate::manifest::LockEntry;
    let current = LockEntry {
        tool: "wai".to_string(),
        version: "2026.1.0".to_string(),
        artifact_url: "https://example.com/wai.tar.gz".to_string(),
        sha256: "abc".to_string(),
        recorded_epoch_day: 20_000,
        contract_sha256: None,
    };
    let candidate = ReleaseCandidate {
        version: "2026.1.0".to_string(),
        artifact_url: "https://example.com/wai.tar.gz".to_string(),
        sha256: "abc".to_string(),
        published_epoch_day: 20_000,
    };
    assert!(
        matches!(
            upgrade_decision(&current, &candidate, 14, 20_010),
            Ok(UpgradeDecision::UpToDate)
        ),
        "identical artifact must be UpToDate"
    );
    let newer = ReleaseCandidate {
        version: "2026.2.0".to_string(),
        artifact_url: "https://example.com/wai-2.tar.gz".to_string(),
        sha256: "def".to_string(),
        published_epoch_day: 20_008,
    };
    assert!(
        matches!(
            upgrade_decision(&current, &newer, 14, 20_010),
            Ok(UpgradeDecision::Deferred { .. })
        ),
        "release younger than the cool-down must be Deferred, not installed"
    );
    assert!(
        matches!(
            upgrade_decision(&current, &newer, 14, 20_030),
            Ok(UpgradeDecision::Upgrade { .. })
        ),
        "mature new release must upgrade"
    );
}
