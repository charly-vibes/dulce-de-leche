//! Timeout-immune repo gate runner (`ddl gates run`, DDL-9kz.6).
//!
//! Purpose: 306 'Command timed out' kills in the 60-day window restarted
//! long gates from scratch; this runner spawns the gate detached (setsid
//! supervisor), so the child survives the harness killing ddl, and either
//! streams the gate to completion or degrades to a run handle (pid + log)
//! the next invocation can re-attach to.
//!
//! Responsibilities:
//! - spawn the gate command under `setsid sh -c`, output to a per-run log
//!   file, exit status via a per-run status file (no waitpid coupling)
//! - one live gate per repo: a live run handle refuses a second spawn
//! - wait mode: stream the log, wait (bounded by `--wait-secs`, default
//!   100 < the common 120s harness timeout), exit with the child's status,
//!   print the tail on failure
//! - detach/long-run mode: return the handle immediately; the child keeps
//!   running — killable by pid, reported in the handle JSON
//!
//! Rationale: `sh -c` + status file keeps the supervisor free of libc and
//! portable across unix; the gate command itself stays the repo's own
//! (`just ci` by default) — ddl never owns a gate definition (anti-goal).

use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::error::{DdlError, Result};

/// Default gate command: the single repo gate entry point (§9).
pub const DEFAULT_GATE_CMD: &[&str] = &["just", "ci"];

/// How long ddl waits before releasing to the run handle: below the
/// common 120s harness tool timeout, so ddl is never the process that
/// dies mid-gate.
pub const DEFAULT_WAIT_SECS: u64 = 100;

/// A live (or finished) gate run, persisted at `.ddl/gates/run.json`.
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct RunHandle {
    pub pid: u32,
    pub command: String,
    pub log_path: String,
    pub status_path: String,
    pub started_at: String,
    /// running | exited(<code>)
    pub status: String,
}

fn gates_dir(repo_root: &Path) -> PathBuf {
    repo_root.join(".ddl/gates")
}

fn run_json_path(repo_root: &Path) -> PathBuf {
    gates_dir(repo_root).join("run.json")
}

fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

fn joined_command(cmd: &[String]) -> String {
    cmd.iter()
        .map(|a| sh_quote(a))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Epoch-tagged stem for this run's log and status files.
fn run_stem() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("run-{secs}-{}", std::process::id())
}

/// Is the pid alive? (unix: /proc/<pid> presence; elsewhere: false)
fn pid_alive(pid: u32) -> bool {
    Path::new(&format!("/proc/{pid}")).exists()
}

/// Read the recorded exit code from a finished run's status file.
fn recorded_exit(status_path: &Path) -> Option<i32> {
    std::fs::read_to_string(status_path)
        .ok()
        .and_then(|s| s.trim().parse::<i32>().ok())
}

/// The live handle, if a gate is still running in this repo: the handle
/// file exists, its pid is alive, and no status file has landed yet.
/// Also cleans a stale handle whose pid is gone and whose status landed.
pub fn live_run(repo_root: &Path) -> Option<RunHandle> {
    let path = run_json_path(repo_root);
    let text = std::fs::read_to_string(&path).ok()?;
    let handle: RunHandle = serde_json::from_str(&text).ok()?;
    if let Some(code) = recorded_exit(Path::new(&handle.status_path)) {
        // Finished since the last run: drop the handle file.
        let _ = std::fs::remove_file(&path);
        return Some(RunHandle {
            status: format!("exited({code})"),
            ..handle
        });
    }
    if !pid_alive(handle.pid) {
        // Supervisor died without a status file (killed -9): stale.
        let _ = std::fs::remove_file(&path);
        return None;
    }
    Some(handle)
}

/// Spawn the gate detached and persist the run handle. The child runs
/// `setsid sh -c '<cmd> > <log> 2>&1; printf %s "$?" > <status>'` so it
/// survives ddl being killed, and its exit lands in a file no waitpid can
/// lose.
struct RunPaths {
    log: PathBuf,
    status: PathBuf,
}

fn create_run_paths(repo_root: &Path) -> Result<RunPaths> {
    let dir = gates_dir(repo_root);
    std::fs::create_dir_all(&dir).map_err(DdlError::Io)?;
    let stem = run_stem();
    Ok(RunPaths {
        log: dir.join(format!("{stem}.log")),
        status: dir.join(format!("{stem}.status")),
    })
}

fn spawn_supervisor(repo_root: &Path, cmd: &[String], paths: &RunPaths) -> Result<u32> {
    let script = format!(
        "{} > {} 2>&1; printf '%s' \"$?\" > {}",
        joined_command(cmd),
        sh_quote(&paths.log.to_string_lossy()),
        sh_quote(&paths.status.to_string_lossy()),
    );
    let child = std::process::Command::new("setsid")
        .arg("sh")
        .arg("-c")
        .arg(&script)
        .current_dir(repo_root)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| DdlError::Other(format!("gates run: spawning detached gate: {e}")))?;
    Ok(child.id())
}

fn persist_handle(repo_root: &Path, handle: &RunHandle) -> Result<()> {
    std::fs::write(
        run_json_path(repo_root),
        serde_json::to_string_pretty(handle).map_err(|e| DdlError::Other(e.to_string()))?,
    )
    .map_err(DdlError::Io)
}

fn spawn_gate(repo_root: &Path, cmd: &[String]) -> Result<RunHandle> {
    let paths = create_run_paths(repo_root)?;
    let pid = spawn_supervisor(repo_root, cmd, &paths)?;

    // setsid may have re-parented the supervisor; the pid is valid either
    // way. Let the process group settle before reporting it live.
    std::thread::sleep(Duration::from_millis(150));

    let handle = RunHandle {
        pid,
        command: cmd.join(" "),
        log_path: paths.log.to_string_lossy().to_string(),
        status_path: paths.status.to_string_lossy().to_string(),
        started_at: humantime_now(),
        status: "running".to_string(),
    };
    persist_handle(repo_root, &handle)?;
    Ok(handle)
}

fn humantime_now() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .map(|s| format!("{s}"))
        .unwrap_or_default()
}

/// Stream new log bytes to stdout since `offset`; returns the new offset.
fn stream_new_output(log_path: &Path, offset: u64) -> u64 {
    let Ok(meta) = std::fs::metadata(log_path) else {
        return offset;
    };
    let len = meta.len();
    if len <= offset {
        return offset;
    }
    let Ok(mut f) = std::fs::File::open(log_path) else {
        return offset;
    };
    use std::io::{Read, Seek, SeekFrom, Write};
    if f.seek(SeekFrom::Start(offset)).is_err() {
        return offset;
    }
    let mut buf = vec![0u8; (len - offset) as usize];
    let n = f.read(&mut buf).unwrap_or(0);
    if n > 0 {
        let mut out = std::io::stdout().lock();
        let _ = out.write_all(&buf[..n]);
        let _ = out.flush();
    }
    offset + n as u64
}

/// Re-attach to a live run: (optionally) stream the log and wait for the
/// status file. Returns the recorded exit code.
fn wait_on(handle: &RunHandle, deadline: Option<Instant>, stream: bool) -> Result<Option<i32>> {
    let log_path = PathBuf::from(&handle.log_path);
    let status_path = PathBuf::from(&handle.status_path);
    let mut offset = 0u64;
    loop {
        if stream {
            offset = stream_new_output(&log_path, offset);
        }
        if let Some(code) = recorded_exit(&status_path) {
            if stream {
                stream_new_output(&log_path, offset);
            }
            return Ok(Some(code));
        }
        if !pid_alive(handle.pid) && recorded_exit(&status_path).is_none() {
            return Ok(None); // supervisor vanished; handle it at the caller
        }
        if let Some(d) = deadline
            && Instant::now() >= d
        {
            return Ok(None); // release point
        }
        std::thread::sleep(Duration::from_millis(400));
    }
}

/// What a `gates run` invocation ended with. The caller decides rendering
/// and process exit codes (the lib never exits the process itself).
#[derive(Debug, Clone)]
pub enum GateOutcome {
    /// The gate finished inside the wait budget (or a re-attach caught it
    /// finishing): exit with `code`.
    Exited { code: i32, handle: RunHandle },
    /// Detach or exhausted wait budget: the child keeps running detached.
    Released { handle: RunHandle },
}

/// Run the gate (or re-attach to a live one).
///
/// - a command given while a gate is live: refuse the concurrent spawn
/// - no command while live (or `--detach`): attach/report the handle
/// - `--wait-secs` bounds the wait; past it (or `--detach`) the run is
///   released to its handle and the child keeps running
/// - `stream` pipes the log into stdout as it grows (human mode; JSON
///   consumers get the log path instead of interleaved raw output)
fn normalize_cmd(cmd: &[String]) -> Vec<String> {
    if cmd.is_empty() {
        DEFAULT_GATE_CMD
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
    } else {
        cmd.to_vec()
    }
}

fn concurrent_refusal(live: &RunHandle) -> DdlError {
    DdlError::Other(format!(
        "gate already running (pid {}, started {}): log {} — \
         re-run without a command to re-attach, or kill {}",
        live.pid, live.started_at, live.log_path, live.pid
    ))
}

/// Per-run gate options: whether to detach immediately, how long to wait
/// before releasing, and whether the log streams to stdout (human mode).
pub struct GateOpts {
    detach: bool,
    wait_secs: u64,
    stream: bool,
}

impl GateOpts {
    pub fn new(detach: bool, wait_secs: u64, stream: bool) -> Self {
        Self {
            detach,
            wait_secs,
            stream,
        }
    }
}

/// Attach to a live run: refuse an explicit new command, report the
/// handle on detach, or wait (bounded) for the status file.
fn attach_live_run(live: RunHandle, cmd: &[String], opts: GateOpts) -> Result<GateOutcome> {
    if !cmd.is_empty() {
        return Err(concurrent_refusal(&live));
    }
    if opts.detach {
        return Ok(GateOutcome::Released { handle: live });
    }
    let deadline = Instant::now() + Duration::from_secs(opts.wait_secs);
    let code = wait_on(&live, Some(deadline), opts.stream)?;
    match code {
        Some(c) => Ok(finished_run(c, live)),
        None => Ok(GateOutcome::Released { handle: live }),
    }
}

fn finished_run(code: i32, mut handle: RunHandle) -> GateOutcome {
    handle.status = format!("exited({code})");
    GateOutcome::Exited { code, handle }
}

fn run_fresh_gate(repo_root: &Path, cmd: &[String], opts: GateOpts) -> Result<GateOutcome> {
    let handle = spawn_gate(repo_root, cmd)?;
    if opts.detach || opts.wait_secs == 0 {
        return Ok(GateOutcome::Released { handle });
    }
    let deadline = Instant::now() + Duration::from_secs(opts.wait_secs);
    let code = wait_on(&handle, Some(deadline), opts.stream)?;
    match code {
        Some(c) => Ok(finished_run(c, handle)),
        None => Ok(GateOutcome::Released { handle }),
    }
}

pub fn run_gate(repo_root: &Path, cmd: &[String], opts: GateOpts) -> Result<GateOutcome> {
    let repo_root = repo_root
        .canonicalize()
        .unwrap_or_else(|_| repo_root.to_path_buf());
    let cmd = normalize_cmd(cmd);

    if let Some(live) = live_run(&repo_root)
        && live.status == "running"
    {
        return attach_live_run(live, &cmd, opts);
    }
    run_fresh_gate(&repo_root, &cmd, opts)
}
