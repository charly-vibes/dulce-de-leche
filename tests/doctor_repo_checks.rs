//! Integration tests for the `ddl.repo.*` status-pipeline doctor checks
//! (DDL-852c tasks 2.2–2.4): `status-fresh` (content-diff staleness),
//! `beads-block`, `brief-drift-gate`, `unmanaged-status-artifacts`, the
//! separate `repo` JSON key / human section, tool-count separation, and
//! error-identity parity between `--json` and human output.

mod common;

use assert_cmd::Command;
use dulce_de_leche::gates::{GateCommands, render_pre_commit_block};
use serde_json::{Value, json};
use std::path::Path;
use std::time::Duration;

/// Timeout for each test.
const CMD_TIMEOUT: Duration = Duration::from_secs(120);

const ORDERING: &str = "Run `ddl status --refresh` — it regenerates";

/// Committed (tracked) fixture bytes: the content side of every
/// content-diff comparison.
const FIXTURE_BRIEF_MD: &str = "# Brief\n\n## Health\n\nall clear\n";
const FIXTURE_BRIEF_JSON: &str = r#"{"ok":true,"schema_version":"brief.v1","generated_at":"2026-11-27T08:00:00Z","data":{"health":{"reason":"all clear"}}}"#;
const FIXTURE_BEADS_EXPORT: &str = "{\"id\":\"ddl-1\",\"title\":\"a\"}\n";
const FIXTURE_BD_LIST: &str = r#"{"open": 1, "blocked": 0}"#;

/// Drifting fixture content — differs from the committed bytes above, so
/// the content-diff verdict must read `stale`.
const DRIFT_BRIEF_MD: &str = "# Brief\n\n## Health\n\nCHANGED\n";
const DRIFT_BEADS_EXPORT: &str = "{\"id\":\"ddl-2\",\"title\":\"b\"}\n";

/// Fixture-isolated git (forced empty local hooks dir; no hook env).
fn git(root: &Path, args: &[&str]) {
    let hooks_dir = root.join(".githooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();
    let out = std::process::Command::new("git")
        .arg("-c")
        .arg(format!("core.hooksPath={}", hooks_dir.display()))
        .args(args)
        .current_dir(root)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?} failed: {}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

fn write_exec(dir: &Path, name: &str, body: &str) {
    let path = dir.join(name);
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    #[cfg(not(unix))]
    let _ = body;
}

fn shim_path(bin: &Path) -> String {
    let system = std::env::var("PATH").unwrap_or_default();
    format!("{}:{system}", bin.display())
}

struct Fixture {
    dir: tempfile::TempDir,
    bin: Option<tempfile::TempDir>,
    drifting: bool,
}

/// Minimal git repo with a `.ddl` manifest (tools empty by default).
fn fixture() -> Fixture {
    fixture_inner(false, false)
}

fn fixture_with_drift() -> Fixture {
    fixture_inner(true, true)
}

fn fixture_with_fresh_producers() -> Fixture {
    fixture_inner(true, false)
}

fn fixture_inner(shims: bool, drifting: bool) -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    git(root, &["init"]);
    let ddl = root.join(".ddl");
    std::fs::create_dir_all(&ddl).unwrap();
    std::fs::write(
        ddl.join("manifest.json"),
        serde_json::to_string_pretty(&json!({
            "ddl_version": "0.3.0",
            "migration_state": "none",
            "tools": {},
        }))
        .unwrap(),
    )
    .unwrap();

    let bin = if shims {
        let bin = tempfile::tempdir().unwrap();
        // podras shim: `brief --json` prints the envelope; `brief --write
        // --out-dir D` renders the pair. Content is env-driven so the same
        // shim serves the fresh and the drifting fixtures.
        write_exec(
            bin.path(),
            "podras",
            r#"
dest="."
prev=""
for a in "$@"; do
  if [ "$prev" = "--out-dir" ] && [ -n "$a" ]; then dest="$a"; fi
  prev="$a"
done
case " $* " in
  *" --write "*)
    mkdir -p "$dest"
    printf '%s' "$PODRAS_FIXTURE_MD" > "$dest/BRIEF.md"
    printf '%s' "$PODRAS_FIXTURE_JSON" > "$dest/BRIEF.json"
    ;;
  *)
    printf '%s' "$PODRAS_FIXTURE_JSON"
    ;;
esac
"#,
        );
        // bd shim: `list --json` counts, `export` the export bytes.
        write_exec(
            bin.path(),
            "bd",
            r#"
case "$1" in
  list) printf '%s' "$BD_FIXTURE_LIST" ;;
  export) printf '%s' "$BD_FIXTURE_EXPORT" ;;
esac
"#,
        );
        Some(bin)
    } else {
        None
    };

    Fixture { dir, bin, drifting }
}

impl Fixture {
    fn root(&self) -> &Path {
        self.dir.path()
    }

    /// Commit tracked feed stores so the record becomes "owed" and the
    /// content-diff has committed artifacts to compare against.
    fn commit_stores(&self) {
        let root = self.root();
        std::fs::write(root.join("BRIEF.md"), FIXTURE_BRIEF_MD).unwrap();
        std::fs::write(root.join("BRIEF.json"), FIXTURE_BRIEF_JSON).unwrap();
        std::fs::create_dir_all(root.join(".beads")).unwrap();
        std::fs::write(root.join(".beads/issues.jsonl"), FIXTURE_BEADS_EXPORT).unwrap();
        git(root, &["add", "-A"]);
        git(
            root,
            &[
                "-c",
                "user.name=ddl-test",
                "-c",
                "user.email=ddl@test.local",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "-m",
                "fixture stores",
            ],
        );
    }

    /// Mark podras/bd installed in the manifest so the obtainment runs —
    /// ALWAYS with PATH shims installed (`self.bin`), never the host's real
    /// producers: the shims' content is what the verdict compares against.
    fn install_producers(&self) {
        let root = self.root();
        assert!(
            self.bin.is_some(),
            "install_producers requires the shim fixture"
        );
        std::fs::write(
            root.join(".ddl/manifest.json"),
            serde_json::to_string_pretty(&json!({
                "ddl_version": "0.3.0",
                "migration_state": "none",
                "tools": {
                    "podras": {"installed": "0.0.0", "source": "fixture", "status": "installed", "compatible": "true"},
                    "bd": {"installed": "0.0.0", "source": "fixture", "status": "installed", "compatible": "true"}
                },
            }))
            .unwrap(),
        )
        .unwrap();
    }

    /// Write a minimal tolerant record whose payloads mirror the fixture
    /// producers exactly — so the content-diff verdicts turn on the
    /// committed-repo files vs the shim output, never on this payload.
    fn write_record(&self) {
        let record = json!({
            "ok": true,
            "envelope_version": "0.1",
            "schema_version": "repo-status.v1",
            "generated_at": "2026-11-27T08:00:00Z",
            "data": {"feeds": [
                {"name":"podras","observed_at":"2026-11-27T08:00:00Z","staleness":"fresh","payload": serde_json::from_str::<Value>(FIXTURE_BRIEF_JSON).unwrap()},
                {"name":"bd","observed_at":"2026-11-27T08:00:00Z","staleness":"fresh","payload": serde_json::from_str::<Value>(FIXTURE_BD_LIST).unwrap()}
            ]}
        });
        std::fs::write(
            self.root().join(".ddl/status.json"),
            serde_json::to_string_pretty(&record).unwrap(),
        )
        .unwrap();
    }

    fn doctor_cmd(&self) -> Command {
        let mut cmd = Command::cargo_bin("ddl").unwrap();
        common::clean_git_env(&mut cmd);
        cmd.current_dir(self.root());
        if let Some(bin) = &self.bin {
            let md = if self.drifting {
                DRIFT_BRIEF_MD
            } else {
                FIXTURE_BRIEF_MD
            };
            let export = if self.drifting {
                DRIFT_BEADS_EXPORT
            } else {
                FIXTURE_BEADS_EXPORT
            };
            cmd.env("PODRAS_FIXTURE_MD", md);
            cmd.env("PODRAS_FIXTURE_JSON", FIXTURE_BRIEF_JSON);
            cmd.env("BD_FIXTURE_LIST", FIXTURE_BD_LIST);
            cmd.env("BD_FIXTURE_EXPORT", export);
            cmd.env("PATH", shim_path(bin.path()));
        }
        cmd.arg("doctor").timeout(CMD_TIMEOUT);
        cmd
    }
}

/// Run `ddl doctor --json` and return the full data payload.
fn doctor_data(cmd: &mut Command) -> Value {
    cmd.arg("--json");
    let out = cmd.assert().success().get_output().stdout.clone();
    let envelope: Value = serde_json::from_slice(&out).expect("valid JSON envelope");
    envelope["data"].clone()
}

fn repo_checks(data: &Value) -> Vec<Value> {
    data["repo"]["checks"]
        .as_array()
        .cloned()
        .unwrap_or_default()
}

fn find<'a>(checks: &'a [Value], name: &str) -> Option<&'a Value> {
    checks.iter().find(|c| c["check"] == json!(name))
}

/// Assert no null values inside `value`.
fn assert_no_nulls(value: &Value, path: &str) {
    match value {
        Value::Null => panic!("unexpected null at {path}"),
        Value::Object(map) => {
            for (k, v) in map {
                assert_no_nulls(v, &format!("{path}/{k}"));
            }
        }
        Value::Array(items) => {
            for (i, v) in items.iter().enumerate() {
                assert_no_nulls(v, &format!("{path}[{i}]"));
            }
        }
        _ => {}
    }
}

// ===================== status-fresh =====================

/// Missing record + tracked brief/beads stores → FAIL, with the ordering
/// sentence and the exact refresh fix.
#[test]
fn status_fresh_fails_when_record_missing_but_stores_tracked() {
    let fixture = fixture();
    fixture.commit_stores();
    let data = doctor_data(&mut fixture.doctor_cmd());
    let checks = repo_checks(&data);
    let fresh = find(&checks, "ddl.repo.status-fresh").expect("status-fresh finding");
    assert_eq!(fresh["level"], json!("fail"));
    let detail = fresh["detail"].as_str().unwrap();
    assert!(
        detail.contains("no status record"),
        "detail must name the missing record: {detail}"
    );
    assert!(
        detail.contains(ORDERING),
        "detail must carry the ordering sentence: {detail}"
    );
    assert_eq!(fresh["fix"], json!("ddl status --refresh"));
}

/// Missing record + nothing tracked → PASS (nothing to orient on).
#[test]
fn status_fresh_passes_when_nothing_tracked() {
    let fixture = fixture();
    let data = doctor_data(&mut fixture.doctor_cmd());
    let checks = repo_checks(&data);
    let fresh = find(&checks, "ddl.repo.status-fresh").expect("status-fresh runs in a repo");
    assert_eq!(fresh["level"], json!("pass"));
}

/// A feed stale by CONTENT DIFF (shim output differs from the committed
/// bytes after volatile-field normalization) → FAIL naming the feeds.
#[test]
fn status_fresh_fails_on_content_diff_staleness() {
    let fixture = fixture_with_drift();
    fixture.commit_stores();
    fixture.install_producers();
    fixture.write_record();
    let data = doctor_data(&mut fixture.doctor_cmd());
    let checks = repo_checks(&data);
    let fresh = find(&checks, "ddl.repo.status-fresh").expect("status-fresh finding");
    assert_eq!(fresh["level"], json!("fail"));
    let detail = fresh["detail"].as_str().unwrap();
    assert!(
        detail.contains("stale by content diff"),
        "detail must state the content-diff verdict: {detail}"
    );
    assert!(
        detail.contains("podras"),
        "stale podras feed named: {detail}"
    );
    assert!(detail.contains("bd"), "stale bd feed named: {detail}");
    assert!(
        detail.contains(ORDERING),
        "detail carries the ordering sentence: {detail}"
    );
}

/// Byte-identical content (the whisper-class case): the tracked store
/// unchanged means fresh, whatever the timestamps say.
#[test]
fn status_fresh_passes_when_content_matches() {
    let fixture = fixture_with_fresh_producers();
    fixture.commit_stores();
    fixture.install_producers();
    fixture.write_record();
    let data = doctor_data(&mut fixture.doctor_cmd());
    let checks = repo_checks(&data);
    let fresh = find(&checks, "ddl.repo.status-fresh").expect("status-fresh runs");
    assert_eq!(
        fresh["level"],
        json!("pass"),
        "identical content must be fresh: {fresh}"
    );
}

// ===================== beads-block =====================

/// A beads block claiming the export is the source of truth → FAIL, with
/// the file and the incorrect claim named, fix `ddl init --gates`.
#[test]
fn beads_block_contradiction_is_flagged() {
    let fixture = fixture();
    std::fs::write(
        fixture.root().join("AGENTS.md"),
        "<!-- BEADS-SEMANTICS:START -->\n## Beads storage semantics\n\n- `.beads/issues.jsonl` is the source of truth for issues.\n<!-- BEADS-SEMANTICS:END -->\n",
    )
    .unwrap();
    let data = doctor_data(&mut fixture.doctor_cmd());
    let checks = repo_checks(&data);
    let block = find(&checks, "ddl.repo.beads-block").expect("beads-block finding");
    assert_eq!(block["level"], json!("fail"));
    let detail = block["detail"].as_str().unwrap();
    assert!(
        detail.contains("AGENTS.md"),
        "detail must name the file: {detail}"
    );
    assert!(
        detail.contains("source of truth"),
        "detail must name the incorrect claim: {detail}"
    );
    assert_eq!(block["fix"], json!("ddl init --gates"));
}

/// The canonical block (states export/sync semantics) → PASS.
#[test]
fn beads_block_canonical_passes() {
    let fixture = fixture();
    std::fs::write(
        fixture.root().join("AGENTS.md"),
        dulce_de_leche::installer::beads_semantics_block(),
    )
    .unwrap();
    let data = doctor_data(&mut fixture.doctor_cmd());
    let checks = repo_checks(&data);
    let block = find(&checks, "ddl.repo.beads-block").expect("beads-block runs");
    assert_eq!(block["level"], json!("pass"), "{block}");
}

/// No beads block at all → PASS (writing the block is the wiring's job).
#[test]
fn beads_block_absent_passes() {
    let fixture = fixture();
    let data = doctor_data(&mut fixture.doctor_cmd());
    let checks = repo_checks(&data);
    let block = find(&checks, "ddl.repo.beads-block").expect("beads-block runs");
    assert_eq!(block["level"], json!("pass"), "{block}");
}

// ===================== brief-drift-gate =====================

/// A committed render without the gate entry → FAIL.
#[test]
fn brief_drift_gate_absent_where_render_committed_fails() {
    let fixture = fixture();
    fixture.commit_stores();
    // A managed pre-commit span WITHOUT the ddl-brief-drift entry.
    std::fs::write(
        fixture.root().join("lefthook.yml"),
        format!(
            "pre-commit:\n  commands:\n{}\n",
            render_pre_commit_block(&GateCommands::default())
        ),
    )
    .unwrap();
    let data = doctor_data(&mut fixture.doctor_cmd());
    let checks = repo_checks(&data);
    let gate = find(&checks, "ddl.repo.brief-drift-gate").expect("brief-drift-gate finding");
    assert_eq!(gate["level"], json!("fail"));
    let detail = gate["detail"].as_str().unwrap();
    assert!(
        detail.contains("brief render is committed"),
        "detail must state the committed render: {detail}"
    );
    assert!(
        detail.contains("ddl-brief-drift"),
        "detail must name the missing gate: {detail}"
    );
    assert_eq!(gate["fix"], json!("ddl init --gates"));
}

/// With the gate entry armed in the managed span → PASS.
#[test]
fn brief_drift_gate_armed_passes() {
    let fixture = fixture();
    fixture.commit_stores();
    let mut commands = GateCommands::default();
    commands.brief_drift = "test -f BRIEF.md".to_string();
    std::fs::write(
        fixture.root().join("lefthook.yml"),
        format!(
            "pre-commit:\n  commands:\n{}\n",
            render_pre_commit_block(&commands)
        ),
    )
    .unwrap();
    let data = doctor_data(&mut fixture.doctor_cmd());
    let checks = repo_checks(&data);
    let gate = find(&checks, "ddl.repo.brief-drift-gate").expect("brief-drift-gate runs");
    assert_eq!(gate["level"], json!("pass"), "{gate}");
}

/// No committed render → PASS ("no render, no gate", DDL-4jvz).
#[test]
fn brief_drift_gate_passes_without_render() {
    let fixture = fixture();
    let data = doctor_data(&mut fixture.doctor_cmd());
    let checks = repo_checks(&data);
    let gate = find(&checks, "ddl.repo.brief-drift-gate").expect("brief-drift-gate runs");
    assert_eq!(gate["level"], json!("pass"), "{gate}");
}

// ===================== unmanaged-status-artifacts =====================

/// A hand-written root status artifact → WARN listing it with its
/// last-commit date; never a failure.
#[test]
fn unmanaged_artifacts_warn_with_last_commit_date() {
    let fixture = fixture();
    std::fs::write(fixture.root().join("AGENT-SUMMARY.md"), "# notes\n").unwrap();
    git(fixture.root(), &["add", "-A"]);
    git(
        fixture.root(),
        &[
            "-c",
            "user.name=ddl-test",
            "-c",
            "user.email=ddl@test.local",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-m",
            "artifact",
        ],
    );
    let data = doctor_data(&mut fixture.doctor_cmd());
    let checks = repo_checks(&data);
    let artifact =
        find(&checks, "ddl.repo.unmanaged-status-artifacts").expect("unmanaged-artifacts finding");
    assert_eq!(
        artifact["level"],
        json!("warn"),
        "WARN, never fail: {artifact}"
    );
    let detail = artifact["detail"].as_str().unwrap();
    assert!(
        detail.contains("AGENT-SUMMARY.md"),
        "detail must list the artifact: {detail}"
    );
    assert!(
        detail.contains("last commit"),
        "detail must carry the last-commit date: {detail}"
    );
}

// ===================== separation + strict =====================

/// Repo findings live ONLY under `repo.checks`; introducing one leaves
/// the tool diagnostics and their summary untouched.
#[test]
fn repo_failures_do_not_alter_tool_counts() {
    let fixture = fixture();
    let clean = doctor_data(&mut fixture.doctor_cmd());
    // Introduce a repo finding (contradictory beads block).
    std::fs::write(
        fixture.root().join("AGENTS.md"),
        "<!-- BEADS-SEMANTICS:START -->\n- `.beads/issues.jsonl` is the source of truth.\n<!-- BEADS-SEMANTICS:END -->\n",
    )
    .unwrap();
    let with_failure = doctor_data(&mut fixture.doctor_cmd());

    // Tool diagnostics identical between the two runs.
    assert_eq!(
        clean["diagnostics"], with_failure["diagnostics"],
        "tool diagnostics must not change with repo findings"
    );
    assert_eq!(
        clean["summary"], with_failure["summary"],
        "tool summary must not change with repo findings"
    );
    // WHY: repo findings are keyed under repo.checks — assert that key
    // directly to prove nothing leaked into the tool sections.
    let dirty_repo = repo_checks(&with_failure);
    let block = find(&dirty_repo, "ddl.repo.beads-block").expect("beads-block finding");
    assert_eq!(block["level"], json!("fail"));
    // WHY: repo.summary is the roll-up of repo.checks only — a tool finding
    // must not move it.
    let repo_summary = &with_failure["repo"]["summary"];
    let count = |level: &str| {
        dirty_repo
            .iter()
            .filter(|c| c["level"] == json!(level))
            .count()
    };
    assert_eq!(repo_summary["fail"], json!(count("fail")));
    assert_eq!(repo_summary["warn"], json!(count("warn")));
    assert_eq!(repo_summary["pass"], json!(count("pass")));
}

/// `--strict` keeps treating ddl.repo.* warn/fail as failures (exit 1)
/// even though the findings moved under the `repo` key.
#[test]
fn strict_exits_nonzero_on_repo_finding() {
    let fixture = fixture();
    std::fs::write(
        fixture.root().join("AGENTS.md"),
        "<!-- BEADS-SEMANTICS:START -->\n- `.beads/issues.jsonl` is the source of truth.\n<!-- BEADS-SEMANTICS:END -->\n",
    )
    .unwrap();
    let mut cmd = fixture.doctor_cmd();
    cmd.arg("--json").arg("--strict");
    cmd.assert().failure();
}

// ===================== --fix refresh =====================

/// `doctor --fix` on a stale/missing record runs the obtain-and-write
/// ordering and reports it as an applied fix; the record file exists
/// afterwards.
#[test]
fn doctor_fix_refreshes_missing_record() {
    let fixture = fixture();
    fixture.commit_stores();
    assert!(
        !fixture.root().join(".ddl/status.json").exists(),
        "fixture starts without a record"
    );
    let mut fix_cmd = fixture.doctor_cmd();
    fix_cmd.arg("--fix");
    let data = doctor_data(&mut fix_cmd);
    let record_path = fixture.root().join(".ddl/status.json");
    assert!(
        record_path.exists(),
        "--fix must write the status record via run_refresh"
    );
    let checks = repo_checks(&data);
    let fresh = find(&checks, "ddl.repo.status-fresh").expect("status-fresh entry");
    assert_eq!(
        fresh["level"],
        json!("pass"),
        "record exists and no feed is stale after the refresh: {fresh}"
    );
    assert!(
        fresh["detail"]
            .as_str()
            .unwrap()
            .contains("status record refreshed"),
        "the applied fix must be reported: {fresh}"
    );
    assert_no_nulls(&data, "doctor.data");
}

// ===================== human section + parity =====================

/// Repo checks render under their own `── Repo checks ──` section, and a
/// failing repo check carries the SAME check name, detail, and fix in the
/// human output and in the JSON items (error-identity parity, task 2.4).
#[test]
fn error_identity_parity_between_human_and_json() {
    let fixture = fixture();
    std::fs::write(
        fixture.root().join("AGENTS.md"),
        "<!-- BEADS-SEMANTICS:START -->\n- `.beads/issues.jsonl` is the source of truth.\n<!-- BEADS-SEMANTICS:END -->\n",
    )
    .unwrap();

    let data = doctor_data(&mut fixture.doctor_cmd());
    let json_items: Vec<Value> = repo_checks(&data)
        .into_iter()
        .filter(|c| c["level"] != json!("pass"))
        .collect();
    assert!(
        json_items
            .iter()
            .any(|c| c["check"] == json!("ddl.repo.beads-block")),
        "fixture must produce a beads-block failure: {json_items:?}"
    );

    let mut human = fixture.doctor_cmd();
    human.arg("--human"); // auto-format picks JSON under a piped TTY
    let human_out = human.assert().get_output().stdout.clone();
    let text = String::from_utf8_lossy(&human_out);
    let repo_header = text
        .find("── Repo checks ──")
        .unwrap_or_else(|| panic!("repo section present: {text}"));
    let diag_header = text
        .find("── Diagnostics ──")
        .expect("diagnostics section present");
    assert!(
        diag_header < repo_header,
        "repo section renders after the diagnostics section"
    );

    for item in &json_items {
        let check = item["check"].as_str().unwrap();
        let detail = item["detail"].as_str().unwrap();
        assert!(
            text.contains(check),
            "human output must carry the check name {check}: {text}"
        );
        assert!(
            text.contains(detail),
            "human output must carry the same detail for {check}: {detail}"
        );
        if let Some(fix) = item["fix"].as_str() {
            assert!(
                text.contains(fix),
                "human output must carry the same fix for {check}: {fix}"
            );
        }
    }
}
