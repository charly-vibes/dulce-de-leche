//! Integration tests for the `ddl status` project-status section
//! (DDL-852c task 2.1, openspec ddl-status-single-source-of-truth):
//! record-sourced human rendering, the may-be-stale display heuristic
//! (labeled as such), and the `project` JSON object beside the tool list.

mod common;

use assert_cmd::Command;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Timeout for each test.
const CMD_TIMEOUT: Duration = Duration::from_secs(30);

/// A `generated_at` older than any fixture commit → heuristic flags stale.
const RECORD_OLD: &str = "2020-01-01T00:00:00Z";
/// A `generated_at` newer than any fixture commit → heuristic stays quiet.
const RECORD_FUTURE: &str = "2099-01-01T00:00:00Z";

/// Fixture-isolated git: forced empty local hooks dir (the host's global
/// core.hooksPath must never fire inside a fixture) and no inherited
/// hook-exported env.
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

struct Fixture {
    dir: tempfile::TempDir,
}

/// A git repo with a `.ddl` manifest and COMMITTED (tracked) feed stores —
/// the tracked-store commits give the heuristic real dates to compare
/// against. The record itself is written untracked per test.
fn fixture() -> Fixture {
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

    std::fs::write(root.join("BRIEF.md"), "# Brief\n\nseed\n").unwrap();
    std::fs::create_dir_all(root.join(".beads")).unwrap();
    std::fs::write(root.join(".beads/issues.jsonl"), "{\"id\":\"ddl-1\"}\n").unwrap();

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
            "fixture",
        ],
    );
    Fixture { dir }
}

impl Fixture {
    fn root(&self) -> &Path {
        self.dir.path()
    }

    /// Write a repo-status.v1 record with a podras feed (available, fresh)
    /// and an openspec feed (unavailable with a detail).
    fn write_record(&self, generated_at: &str) {
        let record = json!({
            "ok": true,
            "envelope_version": "0.1",
            "schema_version": "repo-status.v1",
            "generated_at": generated_at,
            "data": {
                "feeds": [
                    {
                        "name": "podras",
                        "observed_at": "2026-11-27T16:59:58Z",
                        "staleness": "fresh",
                        "payload": { "health": { "reason": "all clear" } }
                    },
                    {
                        "name": "openspec",
                        "status": "unavailable",
                        "detail": "no active changes"
                    }
                ]
            }
        });
        std::fs::write(
            self.root().join(".ddl/status.json"),
            serde_json::to_string_pretty(&record).unwrap(),
        )
        .unwrap();
    }

    fn status_cmd(&self) -> Command {
        let mut cmd = Command::cargo_bin("ddl").unwrap();
        common::clean_git_env(&mut cmd);
        cmd.current_dir(self.root());
        cmd.arg("status").timeout(CMD_TIMEOUT);
        cmd
    }
}

/// Recursively assert no null values inside `value`.
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

/// No record: the section renders the suggestion; the tool-health section
/// still renders; exit code unaffected (success).
#[test]
fn no_record_renders_suggestion_and_keeps_tool_health() {
    let fixture = fixture();
    let output = fixture
        .status_cmd()
        .arg("--human") // auto-format picks JSON under a piped TTY
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&output);
    assert!(
        text.contains("── Project status ──"),
        "project-status section missing: {text}"
    );
    assert!(
        text.contains("no status record — run `ddl status --refresh`"),
        "no-record suggestion missing: {text}"
    );
    // The tool-health section is unchanged: header + per-tool lines.
    assert!(
        text.contains("dulce-de-leche — ecosystem status"),
        "tool-health header must still render: {text}"
    );
}

/// Record present: the section renders the record, and a `generated_at`
/// older than the tracked store's last commit shows the may-be-stale
/// flag (labeled as a heuristic) plus the refresh suggestion.
#[test]
fn record_renders_with_may_be_stale_flag() {
    let fixture = fixture();
    fixture.write_record(RECORD_OLD);
    let output = fixture
        .status_cmd()
        .arg("--human") // auto-format picks JSON under a piped TTY
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&output);
    assert!(
        text.contains("record generated 2020-01-01T00:00:00Z"),
        "record generated_at must render: {text}"
    );
    // Per-feed rows: available fresh, unavailable with detail.
    assert!(text.contains("podras"), "podras feed row missing: {text}");
    assert!(text.contains("fresh"), "podras staleness missing: {text}");
    assert!(
        text.contains("unavailable — no active changes"),
        "unavailable feed detail missing: {text}"
    );
    assert!(
        text.contains("may be stale"),
        "may-be-stale flag missing: {text}"
    );
    assert!(
        text.contains("heuristic"),
        "the flag must be labeled a heuristic: {text}"
    );
    assert!(
        text.contains("→ Run: ddl status --refresh"),
        "refresh suggestion missing: {text}"
    );
}

/// The heuristic compares commit dates: a record generated AFTER the last
/// store commit is NOT flagged stale.
#[test]
fn record_newer_than_store_commits_is_not_flagged() {
    let fixture = fixture();
    fixture.write_record(RECORD_FUTURE);
    let output = fixture
        .status_cmd()
        .arg("--human") // auto-format picks JSON under a piped TTY
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&output);
    assert!(
        !text.contains("may be stale"),
        "fresh record must not be flagged stale: {text}"
    );
    assert!(
        !text.contains("→ Run: ddl status --refresh"),
        "no suggestion when the record is not stale: {text}"
    );
}

/// `--json`: `data.project` carries the record summary and per-feed
/// entries beside the tool `sections`, with no nulls anywhere.
#[test]
fn json_project_object_shape() {
    let fixture = fixture();
    fixture.write_record(RECORD_OLD);
    let out = fixture
        .status_cmd()
        .arg("--json")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let envelope: Value = serde_json::from_slice(&out).expect("valid JSON envelope");
    assert_no_nulls(&envelope["data"], "status.data");

    let project = &envelope["data"]["project"];
    let keys: Vec<&str> = project
        .as_object()
        .expect("project object")
        .keys()
        .map(|s| s.as_str())
        .collect();
    assert_eq!(
        keys,
        vec![
            "feeds",
            "generated_at",
            "may_be_stale",
            "precedence",
            "record_present",
            "suggestion"
        ],
        "project object keys: {keys:?}"
    );
    assert_eq!(project["record_present"], json!(true));
    assert_eq!(project["generated_at"], json!(RECORD_OLD));
    assert_eq!(project["may_be_stale"], json!(true));
    assert_eq!(project["suggestion"], json!("ddl status --refresh"));
    assert!(
        project["precedence"]
            .as_str()
            .is_some_and(|p| !p.is_empty()),
        "precedence header must be embedded: {project}"
    );

    let feeds = project["feeds"].as_array().expect("feeds array");
    assert_eq!(feeds.len(), 2, "both feeds present: {feeds:?}");
    // Available feed: name/observed_at/staleness.
    let podras = feeds
        .iter()
        .find(|f| f["name"] == json!("podras"))
        .expect("podras feed");
    let podras_keys: Vec<&str> = podras
        .as_object()
        .unwrap()
        .keys()
        .map(|s| s.as_str())
        .collect();
    assert_eq!(
        podras_keys,
        vec!["name", "observed_at", "staleness"],
        "available feed keys: {podras_keys:?}"
    );
    assert_eq!(podras["staleness"], json!("fresh"));
    assert_eq!(podras["observed_at"], json!("2026-11-27T16:59:58Z"));
    // Unavailable feed: name/status/detail.
    let openspec = feeds
        .iter()
        .find(|f| f["name"] == json!("openspec"))
        .expect("openspec feed");
    let os_keys: Vec<&str> = openspec
        .as_object()
        .unwrap()
        .keys()
        .map(|s| s.as_str())
        .collect();
    assert_eq!(
        os_keys,
        vec!["detail", "name", "status"],
        "unavailable feed keys"
    );
    assert_eq!(openspec["status"], json!("unavailable"));
    assert_eq!(openspec["detail"], json!("no active changes"));
}

/// No-record JSON: `record_present: false`, no `generated_at` key (never
/// null), empty feeds, and the refresh suggestion.
#[test]
fn json_project_object_without_record() {
    let fixture = fixture();
    let out = fixture
        .status_cmd()
        .arg("--json")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let envelope: Value = serde_json::from_slice(&out).expect("valid JSON envelope");
    assert_no_nulls(&envelope["data"], "status.data");

    let project = &envelope["data"]["project"];
    assert_eq!(project["record_present"], json!(false));
    assert_eq!(project["may_be_stale"], json!(false));
    assert_eq!(project["suggestion"], json!("ddl status --refresh"));
    assert!(
        project.get("generated_at").is_none(),
        "generated_at must be absent (never null) without a record: {project}"
    );
    assert_eq!(
        project["feeds"].as_array().map(Vec::len),
        Some(0),
        "no feeds without a record: {project}"
    );
    // The tool sections render beside the project object.
    assert!(
        envelope["data"]["sections"]
            .as_array()
            .is_some_and(|s| !s.is_empty()),
        "tool sections must stay present: {envelope}"
    );
}

/// The human section embeds the same precedence header the JSON carries.
#[test]
fn human_section_embeds_precedence_header() {
    let fixture = fixture();
    fixture.write_record(RECORD_OLD);
    let json_out = fixture
        .status_cmd()
        .arg("--json")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let envelope: Value = serde_json::from_slice(&json_out).unwrap();
    let precedence = envelope["data"]["project"]["precedence"]
        .as_str()
        .expect("precedence string")
        .to_string();

    let human_out = fixture
        .status_cmd()
        .arg("--human") // auto-format picks JSON under a piped TTY
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let human = String::from_utf8_lossy(&human_out);
    assert!(
        human.contains(&precedence),
        "human section must embed the precedence header ({precedence:?}): {human}"
    );
}

/// The record path used is `<repo>/.ddl/status.json` — a record in a
/// NON-repo directory is still found when ddl runs there (no git needed
/// for the display; the heuristic just stays quiet).
#[test]
fn record_without_git_renders_without_heuristic() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
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
    let record_path: PathBuf = ddl.join("status.json");
    std::fs::write(
        &record_path,
        serde_json::to_string_pretty(&json!({
            "ok": true,
            "envelope_version": "0.1",
            "schema_version": "repo-status.v1",
            "generated_at": RECORD_OLD,
            "data": { "feeds": [] }
        }))
        .unwrap(),
    )
    .unwrap();
    let mut cmd = Command::cargo_bin("ddl").unwrap();
    common::clean_git_env(&mut cmd);
    cmd.current_dir(root);
    cmd.arg("status").arg("--human").timeout(CMD_TIMEOUT); // auto-format picks JSON under a piped TTY
    let stdout = cmd.assert().success().get_output().stdout.clone();
    let text = String::from_utf8_lossy(&stdout);
    assert!(
        text.contains("record generated 2020-01-01T00:00:00Z"),
        "record must render outside git: {text}"
    );
    assert!(
        !text.contains("may be stale"),
        "heuristic must stay quiet without git evidence: {text}"
    );
}
