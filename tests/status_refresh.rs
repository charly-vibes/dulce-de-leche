//! Integration tests for `ddl status --refresh` (DDL-76lm, openspec
//! ddl-status-single-source-of-truth, task 4.1 subset: 1.5 test cases c–f).
//!
//! All producers are PATH shims written into a fixture bin dir — the real
//! bd/podras/openspec on this machine are never depended on. Git is real
//! (genesis git spawns it), exercised against a local `git init` fixture.

mod common;

use assert_cmd::Command;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::time::Duration;

/// Timeout for each test.
const CMD_TIMEOUT: Duration = Duration::from_secs(30);

/// Deterministic fixture content — byte-stable across runs so the
/// idempotency assertion is meaningful.
const FIXTURE_BRIEF_MD: &str = "# Brief\n\n## Health\n\nall clear\n\n## Recent\n\nseed\n\n## Sources & freshness\n\ngenerated_at: 2026-11-27T08:00:00Z\n";
const FIXTURE_BRIEF_JSON: &str = r#"{"ok":true,"schema_version":"brief.v1","generated_at":"2026-11-27T08:00:00Z","data":{"health":{"reason":"all clear"},"sources":[{"name":"bd","count":2,"observed_at":"2026-11-27T07:59:59Z"}]}}"#;
const FIXTURE_BEADS_EXPORT: &str =
    "{\"id\":\"ddl-1\",\"title\":\"a\"}\n{\"id\":\"ddl-2\",\"title\":\"b\"}\n";
const FIXTURE_BD_LIST: &str = r#"{"open": 2, "blocked": 0}"#;
const FIXTURE_OPENSPEC_LIST: &str = r#"{"changes":[{"name":"ddl-status-single-source-of-truth"}]}"#;

struct Fixture {
    dir: tempfile::TempDir,
    bin: tempfile::TempDir,
}

fn git(root: &Path, args: &[&str]) {
    let status = std::process::Command::new("git")
        .args(args)
        .current_dir(root)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .status()
        .expect("git must be available for fixtures");
    assert!(status.success(), "git {args:?} failed");
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

/// Build the isolated repo fixture: a git repo with a committed brief
/// pair, a committed beads export, a `.ddl/` manifest marking
/// podras/bd/openspec installed, and PATH shims for the three producers.
fn fixture(with_podras: bool) -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    git(root, &["init"]);
    // Managed volatile files are ignored so the git feed's dirty count is
    // stable across refreshes in these tests (the record itself is
    // tracked in production repos; that is exercised by the unit tests
    // and phase 2, not the write mechanics under test here).
    std::fs::write(root.join(".gitignore"), ".ddl/status.json\n.ddl/*.lock\n").unwrap();

    let ddl = root.join(".ddl");
    std::fs::create_dir_all(&ddl).unwrap();
    let mut tools = serde_json::Map::new();
    for name in ["podras", "bd", "openspec"] {
        if name == "podras" && !with_podras {
            continue;
        }
        tools.insert(
            name.to_string(),
            json!({"installed": "0.0.0", "source": "fixture", "status": "installed", "compatible": "true"}),
        );
    }
    std::fs::write(
        ddl.join("manifest.json"),
        serde_json::to_string_pretty(&json!({
            "ddl_version": "0.3.0",
            "migration_state": "none",
            "tools": tools,
        }))
        .unwrap(),
    )
    .unwrap();

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
            "fixture",
        ],
    );

    let bin = tempfile::tempdir().unwrap();
    // podras shim: `brief --json` prints the fixture envelope; `brief
    // --write [--out-dir D]` renders the fixture pair (into D or cwd);
    // a short sleep widens the write window for the racing-reader test.
    write_exec(
        bin.path(),
        "podras",
        r#"
sleep 0.05
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
    // bd shim: `list --json` counts, `export` the fixture export bytes.
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
    // openspec shim: `list --json` the fixture changes list.
    write_exec(
        bin.path(),
        "openspec",
        r#"printf '%s' "$OPENSPEC_FIXTURE_LIST""#,
    );

    Fixture { dir, bin }
}

impl Fixture {
    fn root(&self) -> &Path {
        self.dir.path()
    }

    fn record_path(&self) -> PathBuf {
        self.dir.path().join(".ddl/status.json")
    }

    fn read_record(&self) -> Value {
        let raw = std::fs::read(self.record_path()).expect("status record must exist");
        serde_json::from_slice(&raw).expect("status record must be valid JSON")
    }

    fn refresh_cmd(&self) -> Command {
        let mut cmd = Command::cargo_bin("ddl").unwrap();
        common::clean_git_env(&mut cmd);
        cmd.current_dir(self.root());
        cmd.env("PATH", shim_path(self.bin.path()));
        cmd.env("PODRAS_FIXTURE_MD", FIXTURE_BRIEF_MD);
        cmd.env("PODRAS_FIXTURE_JSON", FIXTURE_BRIEF_JSON);
        cmd.env("BD_FIXTURE_LIST", FIXTURE_BD_LIST);
        cmd.env("BD_FIXTURE_EXPORT", FIXTURE_BEADS_EXPORT);
        cmd.env("OPENSPEC_FIXTURE_LIST", FIXTURE_OPENSPEC_LIST);
        cmd.arg("status").arg("--refresh");
        cmd.timeout(CMD_TIMEOUT);
        cmd
    }
}

fn shim_path(bin: &Path) -> String {
    let system = std::env::var("PATH").unwrap_or_default();
    format!("{}:{system}", bin.display())
}

/// A record with every volatile timestamp stripped — the comparison unit
/// for the idempotency assertion (timestamps are allowed to change; the
/// verdict is that nothing else does).
fn strip_volatile(mut record: Value) -> Value {
    record.as_object_mut().unwrap().remove("generated_at");
    if let Some(feeds) = record
        .get_mut("data")
        .and_then(|d| d.get_mut("feeds"))
        .and_then(|f| f.as_array_mut())
    {
        for feed in feeds {
            feed.as_object_mut().unwrap().remove("observed_at");
        }
    }
    record
}

/// (c) refresh idempotent on re-run: the second run changes nothing
/// except the volatile timestamps.
#[test]
fn refresh_is_idempotent_on_rerun() {
    let fixture = fixture(true);
    fixture.refresh_cmd().assert().success();
    let first = strip_volatile(fixture.read_record());

    fixture.refresh_cmd().assert().success();
    let second = strip_volatile(fixture.read_record());

    assert_eq!(first, second, "re-running --refresh must be idempotent");
    // Sanity: the record actually carries all four feeds with verdicts.
    let feeds = second["data"]["feeds"].as_array().unwrap().clone();
    let names: Vec<&str> = feeds.iter().filter_map(|f| f["name"].as_str()).collect();
    assert_eq!(names, vec!["podras", "bd", "git", "openspec"]);
    for feed in &feeds {
        assert_eq!(
            feed["status"],
            json!(null),
            "feed {feed:?} must be available"
        );
        assert_eq!(
            feed["staleness"],
            json!("fresh"),
            "feed {feed:?} must be fresh"
        );
    }
}

/// (d) racing-reader atomicity: a reader looping over the record file
/// during repeated refreshes never observes invalid (partial) JSON —
/// every write goes through temp sibling + rename.
#[test]
fn racing_reader_never_sees_partial_record() {
    let fixture = fixture(true);
    let record_path = fixture.record_path();

    let stop = std::sync::Arc::new(AtomicBool::new(false));
    let stop_reader = stop.clone();
    let reader = std::thread::spawn(move || {
        let mut reads = 0u64;
        while !stop_reader.load(AtomicOrdering::Relaxed) {
            if let Ok(raw) = std::fs::read(&record_path) {
                serde_json::from_slice::<Value>(&raw)
                    .expect("record file must never be partially written");
                reads += 1;
            }
        }
        reads
    });

    for _ in 0..4 {
        fixture.refresh_cmd().assert().success();
    }

    stop.store(true, AtomicOrdering::Relaxed);
    let reads = reader.join().expect("reader thread must not panic");
    assert!(reads > 0, "reader should have observed at least one record");
}

/// (e) podras-absent repo: refresh still exits 0 and records the podras
/// feed as `unavailable` with the reason — absence is a fact about the
/// repo, not an error.
#[test]
fn podras_absent_records_unavailable_and_exits_zero() {
    let fixture = fixture(false);
    fixture.refresh_cmd().assert().success();

    let record = fixture.read_record();
    let podras = record["data"]["feeds"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["name"] == json!("podras"))
        .expect("podras feed must be present even when unavailable");
    assert_eq!(podras["status"], json!("unavailable"));
    assert!(
        podras["detail"].as_str().unwrap().contains("not installed"),
        "detail must carry the producer's reason: {podras:?}"
    );
    // The other feeds still answer.
    let bd = record["data"]["feeds"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["name"] == json!("bd"))
        .unwrap();
    assert_eq!(bd["status"], json!(null));
    assert_eq!(bd["payload"]["open"], json!(2));
}

/// (f) no manifest: refresh refuses with exit code 1, `ddl init`
/// guidance, and no record file created.
#[test]
fn refresh_without_manifest_refuses_and_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init"]);
    git(
        dir.path(),
        &[
            "-c",
            "user.name=ddl-test",
            "-c",
            "user.email=ddl@test.local",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--allow-empty",
            "-m",
            "fixture",
        ],
    );

    let mut cmd = Command::cargo_bin("ddl").unwrap();
    common::clean_git_env(&mut cmd);
    cmd.current_dir(dir.path());
    cmd.arg("status").arg("--refresh");
    cmd.timeout(CMD_TIMEOUT);
    cmd.assert()
        .failure()
        .stderr(predicates::str::contains("ddl init"));

    // No record (and no `.ddl/` at all) was created by the refused run.
    assert!(
        !dir.path().join(".ddl").exists(),
        "a refused refresh must not create .ddl/"
    );
}
