//! Integration tests for the brief-drift gate template and the canonical
//! beads-semantics AGENTS.md block (openspec ddl-status-single-source-of-truth
//! tasks 3.1-3.4, DDL-4jvz): content-based arming, ordering self-heal,
//! version-bump messaging, drift repair, and idempotent re-runs.

mod common;

use assert_cmd::Command;
use dulce_de_leche::gates::{Change, WiringReport};
use dulce_de_leche::gates::{
    GateCommands, apply_gates_wiring_with_sad, detect_beads_semantics_block_problems,
    detect_lefthook_gates_problems,
};
use dulce_de_leche::installer::BeadsSemanticsChange::{Rewritten, Unchanged, Written};
use std::path::{Path, PathBuf};
use std::time::Duration;

const CMD_TIMEOUT: Duration = Duration::from_secs(120);

fn ddl_cmd(temp: &tempfile::TempDir) -> Command {
    let mut cmd = Command::cargo_bin("ddl").unwrap();
    common::clean_git_env(&mut cmd);
    cmd.current_dir(temp.path());
    cmd.timeout(CMD_TIMEOUT);
    cmd
}

fn make_executable(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
}

/// A fake `podras`: renders a deterministic BRIEF.md/BRIEF.json pair. The
/// payload is controlled at invocation time by env — `FAKE_PODRAS_VERSION`
/// (the envelope `cli_version` and the markdown rendered-by line) and
/// `FAKE_TICKETS` (simulated upstream content change). Honors the flags the
/// real CLI accepts and the gate uses: `--write`, `--as-of`, `--out-dir`.
const PODRAS_SHIM: &str = r#"#!/usr/bin/env bash
set -eu
out=.
as_of=${FAKE_PODRAS_AS_OF:-2026-11-27}
write=false
while [ $# -gt 0 ]; do
  case $1 in
    --out-dir) out=$2; shift 2 ;;
    --as-of) as_of=$2; shift 2 ;;
    --write) write=true; shift ;;
    *) shift ;;
  esac
done
if [ "$write" != true ]; then
  echo "fake podras: only --write is supported" >&2
  exit 2
fi
mkdir -p "$out"
ts="${as_of}T00:00:00Z"
version="${FAKE_PODRAS_VERSION:-1.0.0}"
cat > "$out/BRIEF.json" <<EOF
{
  "ok": true,
  "envelope_version": "0.1",
  "cli_version": "$version",
  "data": {
    "schema_version": "brief.v1",
    "as_of": "$ts",
    "generated_at": "$ts",
    "tickets": [{ "id": "DDL-1", "title": "shim ticket", "count": ${FAKE_TICKETS:-3} }],
    "sources": [{ "name": "beads", "observed_at": "$ts" }]
  }
}
EOF
cat > "$out/BRIEF.md" <<EOF
# Project status brief

## In flight

- DDL-1 shim ticket (count: ${FAKE_TICKETS:-3})

<!-- rendered by podras $version -->
EOF
"#;

/// Fixture bin dir with the fake podras (and nothing else: sad/bd absent
/// keeps the wiring steps deterministic without PATH races).
fn shim_bin_dir(temp: &tempfile::TempDir) -> PathBuf {
    let bin = temp.path().join("fakebin");
    std::fs::create_dir_all(&bin).unwrap();
    let podras = bin.join("podras");
    std::fs::write(&podras, PODRAS_SHIM).unwrap();
    make_executable(&podras);
    bin
}

/// Fixture-isolated hook config: the host machine's GLOBAL `core.hooksPath`
/// (a lefthook shim dir) must never fire inside a fixture repo — the commit
/// would run a foreign pre-commit hook and fail before any assertion.
///
/// Forcing an empty local hooks dir with `-c` outranks the global setting.
/// The gate under test is invoked explicitly by `run_gate()`, never through
/// a commit hook.
fn git(root: &Path, args: &[&str]) {
    let hooks_dir = root.join(".githooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();
    let out = std::process::Command::new("git")
        .arg("-c")
        .arg(format!("core.hooksPath={}", hooks_dir.display()))
        .args(args)
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?} failed: {}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Commit the current tree state so the render is git-tracked (the arming
/// test is content-based: `git ls-files`, never timestamps).
fn commit_all(root: &Path, msg: &str) {
    git(root, &["add", "-A"]);
    git(
        root,
        &[
            "-c",
            "user.name=test",
            "-c",
            "user.email=test@example.com",
            "commit",
            "-q",
            "-m",
            msg,
        ],
    );
}

/// Render the fake podras pair into `root` (the shim's default --write
/// target is the cwd, like the real CLI).
fn render_commit(root: &Path, bin_dir: &Path, envs: &[(&str, &str)]) {
    let mut cmd = std::process::Command::new(bin_dir.join("podras"));
    cmd.arg("brief").args(["--write"]).current_dir(root);
    cmd.env("PATH", format!("{}:/usr/bin:/bin", bin_dir.display()));
    for (k, v) in envs {
        cmd.env(k, v);
    }
    let out = cmd.output().unwrap();
    assert!(out.status.success(), "shim podras brief --write failed");
    commit_all(root, "brief render");
}

/// Init a fixture repo; `with_render` commits a brief render pair first.
fn fixture_repo(with_render: bool) -> (tempfile::TempDir, PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    let bin_dir = shim_bin_dir(&temp);
    git(temp.path(), &["init", "-q"]);
    if with_render {
        render_commit(temp.path(), &bin_dir, &[]);
    } else {
        std::fs::write(temp.path().join("README.md"), "beads-only repo\n").unwrap();
        commit_all(temp.path(), "init");
    }
    (temp, bin_dir)
}

/// Extract the `ddl-brief-drift` run script from the wired lefthook.yml —
/// the exact command lefthook would execute (Rust-spliced, shell-ready).
fn brief_drift_script(repo: &Path) -> String {
    let yml = std::fs::read_to_string(repo.join("lefthook.yml")).unwrap();
    let mut script = String::new();
    let mut in_entry = false;
    for line in yml.lines() {
        if line.trim() == "ddl-brief-drift:" {
            in_entry = true;
            continue;
        }
        if in_entry {
            if line.trim() == "run: |" {
                continue;
            }
            if line.starts_with("        ") {
                script.push_str(&line[8..]);
                script.push('\n');
            } else if !line.trim().is_empty() {
                break;
            }
        }
    }
    assert!(
        !script.is_empty(),
        "ddl-brief-drift entry not found in lefthook.yml"
    );
    script
}

/// Execute the wired gate command the way lefthook would (sh, repo root
/// cwd, fixture PATH). `envs` vary the fake podras between invocations.
fn run_gate(repo: &Path, bin_dir: &Path, envs: &[(&str, &str)]) -> std::process::Output {
    let script = brief_drift_script(repo);
    let gate_dir = tempfile::tempdir().unwrap();
    let script_path = gate_dir.path().join("gate-under-test.sh");
    std::fs::write(&script_path, script).unwrap();
    let mut cmd = std::process::Command::new("sh");
    cmd.arg(&script_path).current_dir(repo);
    cmd.env("PATH", format!("{}:/usr/bin:/bin", bin_dir.display()));
    for (k, v) in envs {
        cmd.env(k, v);
    }
    cmd.output().unwrap()
}

fn assert_gate_failed(out: &std::process::Output, what: &str) {
    assert!(
        !out.status.success(),
        "{what}: gate must fail, got exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

// ---------------------------------------------------------------------------
// Arming (task 3.1 + 3.2): content-based, never timestamp-based.
// ---------------------------------------------------------------------------

#[test]
fn init_gates_arms_brief_drift_only_with_committed_render() {
    // Repo with a committed brief render → the gate is armed.
    let (temp, bin_dir) = fixture_repo(true);
    ddl_cmd(&temp)
        .env("PATH", format!("{}:/usr/bin:/bin", bin_dir.display()))
        .args(["--human", "init", "--yes", "--no-install", "--gates"])
        .assert()
        .success();
    let yml = std::fs::read_to_string(temp.path().join("lefthook.yml")).unwrap();
    assert!(
        yml.contains("ddl-brief-drift:"),
        "brief-drift gate must be armed when a render is committed:\n{yml}"
    );
    assert!(
        yml.contains("ddl status --refresh"),
        "armed gate must carry the ordering sentence"
    );

    // Beads-only repo (no committed render) → no render gate; the existing
    // export-drift guard covers it ("no render, no gate", task 3.2).
    let (bare, bin_dir) = fixture_repo(false);
    ddl_cmd(&bare)
        .env("PATH", format!("{}:/usr/bin:/bin", bin_dir.display()))
        .args(["--human", "init", "--yes", "--no-install", "--gates"])
        .assert()
        .success();
    let yml = std::fs::read_to_string(bare.path().join("lefthook.yml")).unwrap();
    assert!(
        !yml.contains("ddl-brief-drift"),
        "no render, no gate — beads-only repo must not arm the render gate:\n{yml}"
    );
}

#[test]
fn config_can_disable_or_force_the_brief_drift_entry() {
    // `brief_drift = ""` disables the entry even with a committed render;
    // an explicit command forces it even without one.
    let (temp, _bin_dir) = fixture_repo(true);
    std::fs::create_dir_all(temp.path().join(".ddl")).unwrap();
    std::fs::write(
        temp.path().join(".ddl/config.toml"),
        "install_source = \"auto\"\n\n[gates]\nbrief_drift = \"\"\n",
    )
    .unwrap();
    let report = apply_gates_wiring_with_sad(temp.path(), None).unwrap();
    assert_eq!(report.lefthook, Change::Created);
    assert!(!brief_drift_entry_present(temp.path()));

    let (bare, _bin_dir) = fixture_repo(false);
    std::fs::create_dir_all(bare.path().join(".ddl")).unwrap();
    std::fs::write(
        bare.path().join(".ddl/config.toml"),
        "install_source = \"auto\"\n\n[gates]\nbrief_drift = \"podras brief --write --check\"\n",
    )
    .unwrap();
    apply_gates_wiring_with_sad(bare.path(), None).unwrap();
    let yml = std::fs::read_to_string(bare.path().join("lefthook.yml")).unwrap();
    assert!(yml.contains("podras brief --write --check"));
}

fn brief_drift_entry_present(repo: &Path) -> bool {
    std::fs::read_to_string(repo.join("lefthook.yml"))
        .unwrap()
        .contains("ddl-brief-drift:")
}

// ---------------------------------------------------------------------------
// Gate semantics (task 3.1): drift detection, ordering self-heal, version bump.
// ---------------------------------------------------------------------------

#[test]
fn gate_passes_after_the_ordering_without_manual_retry() {
    // The 3-retry regression: one run of the ordering must be enough.
    let (temp, bin_dir) = fixture_repo(true);
    // Wire the gates (library call with sad injected absent): the render is
    // committed, so the brief-drift entry is armed.
    let report = apply_gates_wiring_with_sad(temp.path(), None).unwrap();
    assert_eq!(report.lefthook, Change::Created);

    // Upstream reality moved on (shim now renders 5 tickets); the committed
    // render still says 3 → drift.
    let out = run_gate(temp.path(), &bin_dir, &[("FAKE_TICKETS", "5")]);
    assert_gate_failed(&out, "stale committed render");

    // WHY: shims the real refresh ordering — brief render regeneration, export update, record rewrite, re-commit.
    render_commit(temp.path(), &bin_dir, &[("FAKE_TICKETS", "5")]);

    let out = run_gate(temp.path(), &bin_dir, &[("FAKE_TICKETS", "5")]);
    assert!(
        out.status.success(),
        "gate must pass after the ordering without manual retry; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn version_bump_failure_names_committed_version_and_ordering() {
    let (temp, bin_dir) = fixture_repo_with_version("0.9.0");

    // Fresh render comes from a newer podras: the JSON cli_version is
    // normalized away, but the markdown rendered-by line differs — the
    // gate must fail WITHOUT calling it hard drift: the message names the
    // committed producing version and the ordering that self-heals it.
    let out = run_gate(temp.path(), &bin_dir, &[]);
    assert_gate_failed(&out, "version-bump diff");

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("0.9.0"),
        "failure must carry the committed cli_version; stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("Run `ddl status --refresh` — it regenerates the brief render (podras brief --write), updates the tracked export (.beads/issues.jsonl), and rewrites the status record; then re-commit."),
        "failure must state the ordering verbatim; stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("self-healing"),
        "version bump must be framed as self-healing, not hard drift; stderr:\n{stderr}"
    );
}

fn fixture_repo_with_version(version: &str) -> (tempfile::TempDir, PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    let bin_dir = shim_bin_dir(&temp);
    git(temp.path(), &["init", "-q"]);
    render_commit(temp.path(), &bin_dir, &[("FAKE_PODRAS_VERSION", version)]);
    apply_gates_wiring_with_sad(temp.path(), None).unwrap();
    (temp, bin_dir)
}

// ---------------------------------------------------------------------------
// Canonical beads-semantics block (task 3.3) + drift detection.
// ---------------------------------------------------------------------------

#[test]
fn contradictory_beads_block_is_rewritten_to_canonical() {
    let temp = tempfile::tempdir().unwrap();
    let canonical = dulce_de_leche::installer::beads_semantics_block();
    // A block (inside ddl's markers) edited to claim the export is the
    // source of truth — the exact contradiction the doctor check will flag.
    let contradictory = canonical.replace(
        "is the committed export, an output of `bd export`",
        "is the source of truth",
    );
    assert_ne!(contradictory, canonical);
    std::fs::write(temp.path().join("AGENTS.md"), contradictory).unwrap();

    // Writer-side drift detection flags it before wiring…
    let drifted_text = std::fs::read_to_string(temp.path().join("AGENTS.md")).unwrap();
    assert!(
        !detect_beads_semantics_block_problems(Some(&drifted_text)).is_empty(),
        "drifted beads-semantics span must be reported"
    );

    // …and the wiring rewrites it in place (drift-repair semantics).
    let report = apply_gates_wiring_with_sad(temp.path(), None).unwrap();
    assert_eq!(report.beads_semantics, Rewritten);
    let text = std::fs::read_to_string(temp.path().join("AGENTS.md")).unwrap();
    assert!(
        !text.contains("is the source of truth"),
        "contradictory claim must be gone:\n{text}"
    );
    assert!(
        text.contains(&canonical),
        "canonical statement must be in place:\n{text}"
    );
    assert!(detect_beads_semantics_block_problems(Some(&text)).is_empty());
}

#[test]
fn canonical_beads_block_states_the_storage_semantics() {
    let block = dulce_de_leche::installer::beads_semantics_block();
    assert!(block.contains("<!-- BEADS-SEMANTICS:START -->"));
    assert!(block.contains("<!-- BEADS-SEMANTICS:END -->"));
    assert!(block.contains("per-machine"));
    assert!(block.contains("committed export"));
    assert!(block.contains("bd export"));
    assert!(block.contains("Git is the sync mechanism"));
    assert!(block.contains("no dolt remote"));
    assert!(block.contains("bd dolt push"));
    // WHY: the doctor check (later phase) flags "source of truth" applied
    // to the export — the canonical block must never say it.
    assert!(!block.to_lowercase().contains("source of truth"));
}

// ---------------------------------------------------------------------------
// Idempotence + lefthook drift detection over the new entry.
// ---------------------------------------------------------------------------

#[test]
fn second_wiring_run_leaves_blocks_unchanged() {
    let (temp, _bin_dir) = fixture_repo(true);
    let first: WiringReport = apply_gates_wiring_with_sad(temp.path(), None).unwrap();
    assert_eq!(first.lefthook, Change::Created);
    assert_eq!(first.beads_semantics, Written);

    let second = apply_gates_wiring_with_sad(temp.path(), None).unwrap();
    assert_eq!(second.lefthook, Change::Unchanged);
    assert_eq!(
        second.session_protocol,
        dulce_de_leche::installer::SessionProtocolChange::Unchanged
    );
    assert_eq!(second.beads_semantics, Unchanged);
    assert!(second.gitignore_added.is_empty());
}

#[test]
fn detect_flags_drifted_brief_drift_span_and_beads_block() {
    // Armed pre-commit span (brief-drift included) → clean; tampering one
    // script line inside the span → drift reported.
    let commands = GateCommands {
        brief_drift: dulce_de_leche::gates::DEFAULT_BRIEF_DRIFT.to_string(),
        ..GateCommands::default()
    };
    // WHY: detect_lefthook_gates_problems checks BOTH hooks, so the
    // synthetic yml must carry the pre-push managed block too — a missing
    // pre-push span is itself a reported problem, and must not mask the
    // round-trip.
    let block = dulce_de_leche::gates::render_pre_commit_block(&commands);
    let pre_push = dulce_de_leche::gates::render_pre_push_block(&commands);
    let yml = format!("pre-commit:\n  commands:\n{block}\n\npre-push:\n  commands:\n{pre_push}\n");
    assert!(
        detect_lefthook_gates_problems(Some(&yml), &commands).is_empty(),
        "a freshly rendered armed block must round-trip through detect clean"
    );

    let tampered = yml.replace("match a fresh render", "TAMPERED");
    assert_ne!(tampered, yml);
    let problems = detect_lefthook_gates_problems(Some(&tampered), &commands);
    assert!(
        problems
            .iter()
            .any(|p| p.contains("pre-commit managed block has drifted")),
        "drifted brief-drift span must be reported: {problems:?}"
    );
}

// ---------------------------------------------------------------------------
// Dogfood (task 3.4 in-repo part): this repo's own lefthook.yml migrated.
// ---------------------------------------------------------------------------

#[test]
fn this_repo_lefthook_yml_matches_the_emitted_template() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let content = std::fs::read_to_string(root.join("lefthook.yml")).unwrap();
    // The hand-rolled fork must be gone.
    assert!(
        !content.contains("scripts/guards/brief-drift.sh"),
        "the hand-rolled brief-drift hook must be migrated to the ddl template"
    );
    // WHY: managed spans must match this repo's effective gate rendering — brief-drift is armed here (BRIEF.md/BRIEF.json tracked).
    let commands = GateCommands::for_repo(&root);
    assert!(
        commands.brief_drift.contains("ddl status --refresh"),
        "the gate must be armed for this repo (committed render)"
    );
    let problems = detect_lefthook_gates_problems(Some(&content), &commands);
    assert!(problems.is_empty(), "lefthook.yml drift: {problems:?}");
}
