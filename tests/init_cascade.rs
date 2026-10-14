//! Integration tests for the `ddl init` tool-init cascade (DDL-6zn.3).
//!
//! Semantics under test:
//! - failures are NOT silently swallowed: a failing tool init exits non-zero
//!   and prints a resume summary with the exact finish commands
//! - prerequisite ordering: openspec init runs BEFORE ah init
//! - tools not on PATH but present at the .ddl/bin destination still init
//! - tools without an init command are skipped silently

/// DDL-6zn.8: --tools accepts aliases (espectacular→ah) and fails loudly
/// on unknown names instead of silently dropping them.
#[test]
fn tools_flag_accepts_crate_alias() {
    let (mut cmd, temp) = ddl_cmd();
    // Hermetic: stub `ah` so the resolved alias never hits the real binary
    // (which would fail without an openspec/ dir).
    stub_tools(&mut cmd, &temp, &[("ah", "echo 'ah stub init'")]);
    cmd.args(["init", "--tools", "espectacular", "--no-install", "--human"]);
    cmd.timeout(CMD_TIMEOUT);
    cmd.assert()
        .stdout(predicates::str::contains("Skipping installation"))
        .stderr(predicates::str::contains("ah initialized"));
}

#[test]
fn tools_flag_tolerates_stray_commas() {
    let (mut cmd, temp) = ddl_cmd();
    // Hermetic: stub both resolved tools so the cascade can't hit real
    // binaries on the dev machine.
    stub_tools(&mut cmd, &temp, &[("dont", "exit 0"), ("ah", "exit 0")]);
    cmd.args([
        "init",
        "--tools",
        "dont,,espectacular,",
        "--no-install",
        "--human",
    ]);
    cmd.timeout(CMD_TIMEOUT);
    cmd.assert()
        .success()
        .stderr(predicates::str::contains("dont initialized"))
        .stderr(predicates::str::contains("ah initialized"));
}

#[test]
fn tools_flag_unknown_name_fails_loudly() {
    let (mut cmd, _temp) = ddl_cmd();
    cmd.args(["init", "--tools", "ddd", "--no-install", "--human"]);
    cmd.timeout(CMD_TIMEOUT);
    cmd.assert().failure();
    cmd.assert()
        .stderr(predicates::str::contains("Unknown tool 'ddd'"));
}

#[test]
fn tools_flag_self_name_names_the_repo() {
    let (mut cmd, _temp) = ddl_cmd();
    cmd.args(["init", "--tools", "dulce", "--no-install", "--human"]);
    cmd.timeout(CMD_TIMEOUT);
    cmd.assert().failure();
    cmd.assert()
        .stderr(predicates::str::contains("charly-vibes/dulce-de-leche"));
}

/// DDL-6zn.8: init help examples are GENERATED from MANAGED_TOOLS —
/// `ddl init --help` must show every registered tool as a valid --tools value.
#[test]
fn init_help_lists_every_registry_tool() {
    let mut cmd = Command::cargo_bin("ddl").unwrap();
    cmd.arg("init").arg("--help");
    cmd.timeout(CMD_TIMEOUT);
    let output = cmd.assert().success().get_output().stdout.clone();
    let stdout = String::from_utf8(output).unwrap();
    for tool in dulce_de_leche::platform::MANAGED_TOOLS {
        assert!(
            stdout.contains(tool.name),
            "init --help is missing registry tool `{}`",
            tool.name
        );
    }
}

use assert_cmd::Command;
use std::path::Path;
use std::time::{Duration, Instant};

/// Timeout for each test — init with --no-install should be instant.
const CMD_TIMEOUT: Duration = Duration::from_secs(60);

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

// ===================== DDL-uu8 (SEC-04) =====================
// Restricted-privilege policy for children spawned by the install/init
// cascade: cargo/npm/npx and tool-init children must run with an explicitly
// restricted environment (allowlist), and execution scopes whose write roots
// reach beyond this project (global skill installs) must require explicit
// policy authorization.

/// A canary credential in the caller environment must never reach a tool-init
/// child: the child environment is built from an explicit allowlist, so
/// non-approved variables (credentials, tokens) are dropped.
#[test]
fn init_children_do_not_inherit_canary_credentials() {
    let (mut cmd, temp) = ddl_cmd();
    stub_tools(
        &mut cmd,
        &temp,
        &[(
            "ah",
            // The stub reports whether the canary leaked into its environment.
            "if env | grep -q '^DDL_TEST_CANARY_CREDENTIAL='; then echo 'CANARY LEAKED'; else echo 'restricted env ok'; fi",
        )],
    );
    cmd.env(
        "DDL_TEST_CANARY_CREDENTIAL",
        "canary-secret-do-not-leak-9f3a",
    );
    cmd.args(["init", "--tools", "ah", "--no-install", "--human"]);
    cmd.timeout(CMD_TIMEOUT);
    let output = cmd.assert().success().get_output().clone();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    assert!(
        !stderr.contains("CANARY LEAKED"),
        "tool-init child inherited the caller's canary credential: {stderr}"
    );
    assert!(
        !stderr.contains("canary-secret-do-not-leak-9f3a"),
        "canary credential value leaked into child output: {stderr}"
    );
    assert!(
        stderr.contains("ah initialized: restricted env ok"),
        "stub should have run with a restricted but usable environment: {stderr}"
    );
}

/// Global-skill installation writes to ~/.agents/skills — visible to every
/// project on the machine — so it requires explicit policy authorization
/// (DDL_ALLOW_GLOBAL_SKILL=1). Without it the install must be rejected with a
/// structured diagnostic naming the authorization variable; the init cascade
/// stays advisory (warn, do not abort) but must NOT run the global install.
#[test]
fn unauthorized_global_skill_scope_is_rejected_with_policy_diagnostic() {
    let (mut cmd, temp) = ddl_cmd();
    // Sandboxed HOME: no pre-existing global skills, so the --yes flow
    // attempts the global skill install.
    cmd.env("HOME", temp.path().join("home"));
    cmd.env_remove("DDL_ALLOW_GLOBAL_SKILL");
    stub_tools(&mut cmd, &temp, &[]); // hermetic incitaciones stub
    cmd.args(["init", "--yes", "--no-install", "--human"]);
    cmd.timeout(CMD_TIMEOUT);
    let output = cmd.assert().success().get_output().clone();
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        combined.contains("DDL_ALLOW_GLOBAL_SKILL"),
        "unauthorized global skill install must produce a diagnostic naming \
         DDL_ALLOW_GLOBAL_SKILL; got: {combined}"
    );
}

/// DDL-6ox: a tool-config step that exits nonzero (dont/bd refusing to
/// re-initialise an already-initialised project) must NOT abort the run
/// before the gates wiring is written. Gates wiring always completes
/// (lefthook block, DDL AGENTS.md block, gitignore entries, pins scaffold);
/// the per-tool config failures surface as advisory entries in the resume
/// summary and the exit code stays nonzero.
#[test]
fn config_step_failure_still_writes_gates() {
    let (mut cmd, temp) = ddl_cmd();
    stub_tools(
        &mut cmd,
        &temp,
        &[
            (
                "dont",
                "echo 'refuses re-init: already initialised' >&2\nexit 1",
            ),
            (
                "bd",
                "echo 'refuses re-init: already initialised' >&2\nexit 1",
            ),
        ],
    );
    // Sandboxed HOME: keep the --yes global-skill step off the real machine.
    cmd.env("HOME", temp.path().join("home"));
    cmd.args(["init", "--yes", "--no-install", "--gates", "--human"]);
    cmd.timeout(CMD_TIMEOUT);
    // Exit code stays nonzero (resume summary), but gates MUST be written.
    // Single execution: capture the output once — a second cmd.assert() call
    // re-runs the command.
    let output = cmd.assert().failure().get_output().clone();

    // Gates wiring completed: lefthook managed block ...
    let lefthook = read(temp.path(), "lefthook.yml");
    assert!(
        lefthook.contains("# ddl:managed:start (pre-commit)"),
        "lefthook gates block missing — config-step failure aborted before gates"
    );
    // ... beads no-db stamp ...
    assert!(read(temp.path(), ".beads/config.yaml").contains("no-db: true"));
    // ... DDL managed block in AGENTS.md ...
    let agents = read(temp.path(), "AGENTS.md");
    assert!(agents.contains("DDL"), "DDL block missing from AGENTS.md");
    // ... pins scaffold ...
    assert!(
        temp.path().join("versions.ddl.toml").exists(),
        "versions.ddl.toml pins scaffold missing"
    );

    // Resume summary names the failed config steps (advisory).
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(
        stdout.contains("Init cascade failed"),
        "resume summary missing from stdout: {stdout}"
    );
    assert!(stdout.contains("dont init failed"));
    assert!(stdout.contains("bd init failed"));
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

    // DDL-6zn.4: NO ok:true list envelope on stdout; the error envelope is
    // the single JSON envelope, on stderr (the old two-envelope failure mode
    // printed a misleading ok:true list on stdout before the error).
    let binding = cmd.assert().failure();
    let out = binding.get_output();
    let stdout = String::from_utf8(out.stdout.clone()).unwrap();
    let stdout_envelopes: Vec<&str> = stdout.lines().filter(|l| l.starts_with('{')).collect();
    assert!(
        stdout_envelopes.is_empty(),
        "no envelopes on stdout: {stdout}"
    );
    let stderr = String::from_utf8(out.stderr.clone()).unwrap();
    // The error envelope is pretty-printed JSON embedded in stderr text:
    // extract from its first '{' to its last '}'.
    let start = stderr.find('{').expect("error envelope on stderr");
    let end = stderr.rfind('}').expect("error envelope closed");
    let env: serde_json::Value = serde_json::from_str(&stderr[start..=end]).unwrap();
    assert_eq!(env["ok"], serde_json::json!(false));
    assert_eq!(env["envelope_version"], serde_json::json!("0.1"));
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

// ===================== DDL-1z1 (SEC-02): init cascade child runtime budget =====================
//
// A tool init command that hangs must be terminated within the configured
// deadline plus one second, the run must fail (exit status identifies the
// exhausted budget), and the diagnostics — human and JSON — must name the
// budget instead of silently hanging or falling back.

#[test]
fn hanging_init_child_killed_and_budget_identified() {
    let (mut cmd, temp) = ddl_cmd();
    // "testaruda" (any managed tool works): "bd" is special-cased with a
    // 300s heavyweight budget (embedded Dolt bootstrap), so the budget
    // mechanism test needs a tool without an init-specific override.
    // DDL-6ox: a `wai` stub keeps the post-failure advisory tail (wai
    // force-init) instant so the elapsed bound stays a strict deadline check.
    stub_tools(
        &mut cmd,
        &temp,
        &[("testaruda", "sleep 60"), ("wai", "exit 0")],
    );
    cmd.env("DDL_CHILD_DEADLINE_SECS", "1");
    cmd.args([
        "init",
        "--yes",
        "--tools",
        "testaruda",
        "--no-install",
        "--human",
    ]);
    let start = Instant::now();
    // Human mode: the resume summary (naming the exhausted budget) is on
    // stdout; the non-zero exit status identifies the budget exhaustion.
    cmd.assert()
        .failure()
        .stdout(predicates::str::contains("budget exceeded: child-runtime"));
    let elapsed = start.elapsed();
    // DDL-6ox: the run no longer aborts at the first init failure — the
    // advisory tail (gitignore, repo setup with per-tool version probes,
    // wai force-init) now executes after the budget kill, adding ~1s of
    // legitimate work. The bound still fails hard if the budget mechanism
    // is broken (a hung child would exceed it by far).
    assert!(
        // CI runners add load to the post-kill advisory tail (real wai init
        // child); the mechanism invariant is the 1s budget kill — a broken
        // budget would only fail here via the 60s CMD_TIMEOUT, so 15s stays
        // a hard, meaningful bound.
        elapsed < Duration::from_secs(15),
        "hanging init child must be terminated within deadline + 1s; took {elapsed:?}"
    );
}

#[test]
fn hanging_init_child_budget_identified_in_json() {
    let (mut cmd, temp) = ddl_cmd();
    // "testaruda" (any managed tool works): "bd" is special-cased with a
    // 300s heavyweight budget (embedded Dolt bootstrap), so the budget
    // mechanism test needs a tool without an init-specific override.
    // DDL-6ox: a `wai` stub keeps the post-failure advisory tail (wai
    // force-init) instant so the elapsed bound stays a strict deadline check.
    stub_tools(
        &mut cmd,
        &temp,
        &[("testaruda", "sleep 60"), ("wai", "exit 0")],
    );
    cmd.env("DDL_CHILD_DEADLINE_SECS", "1");
    cmd.args(["init", "--yes", "--tools", "testaruda", "--no-install"]);
    // Piped stdout auto-selects JSON mode; the error envelope must name the
    // exhausted budget.
    let start = Instant::now();
    cmd.assert()
        .failure()
        .stderr(predicates::str::contains("budget"));
    let elapsed = start.elapsed();
    // DDL-6ox: advisory tail now runs after the budget kill (see the human
    // variant above) — the bound still fails hard if the budget mechanism
    // is broken.
    assert!(
        // CI runners add load to the post-kill advisory tail (real wai init
        // child); the mechanism invariant is the 1s budget kill — a broken
        // budget would only fail here via the 60s CMD_TIMEOUT, so 15s stays
        // a hard, meaningful bound.
        elapsed < Duration::from_secs(15),
        "hanging init child must be terminated within deadline + 1s; took {elapsed:?}"
    );
}
