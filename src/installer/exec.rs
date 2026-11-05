//! Bounded child-process execution and resource budgets (DDL-1z1, SEC-02).

use crate::error::{DdlError, Result};

/// Upper bound on the size of an extracted binary (DDL-4oi). The declared
/// entry size is checked before any byte is written, so a hostile or
/// corrupted archive cannot exhaust the destination disk with staging data.
pub const MAX_BINARY_BYTES: u64 = 512 * 1024 * 1024;

/// Upper bound on the size of a downloaded release asset (DDL-1z1, SEC-02).
/// The declared Content-Length is checked before the first byte is read, and
/// the streamed byte count is re-checked as chunks arrive, so a hostile or
/// corrupted release body cannot exhaust host memory or disk before the
/// policy rejects it.
pub const MAX_RELEASE_ASSET_BYTES: u64 = 512 * 1024 * 1024;

/// Upper bound on output captured from a bootstrap child process
/// (DDL-1z1, SEC-02). Version probes and init commands cannot flood ddl's
/// memory with unbounded output; bytes past the budget are discarded and the
/// child is killed with the budget named in the error.
pub const MAX_CHILD_OUTPUT_BYTES: usize = 4 * 1024 * 1024;

/// Default child runtime deadline in seconds (DDL-1z1, SEC-02). A child that
/// has not exited by the deadline — nor have its descendants — is killed.
/// Controlled fixtures may shorten the deadline via `DDL_CHILD_DEADLINE_SECS`
/// (1–3600); the env override exists for tests, not as a production escape
/// hatch (any value outside the accepted range falls back to the default).
pub const DEFAULT_CHILD_DEADLINE_SECS: u64 = 30;

/// Budget for a single tool's init child. Most tool inits are quick; `bd`
/// bootstrap-creates an embedded Dolt database on first run, which can take
/// well over the default budget on a loaded CI runner — killing it mid-init
/// would leave the store half-created and fail every `ddl init` that touches
/// bd (observed 2026-10-14: CI runner killed bd init at the default 30s).
pub(crate) fn init_deadline_for(tool_name: &str) -> std::time::Duration {
    if tool_name == "bd" {
        std::time::Duration::from_secs(300)
    } else {
        child_deadline()
    }
}

/// Resolve the configured child runtime deadline (DDL-1z1).
pub fn child_deadline() -> std::time::Duration {
    use std::time::Duration;
    std::env::var("DDL_CHILD_DEADLINE_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .filter(|secs| (1..=3600).contains(secs))
        .map(Duration::from_secs)
        .unwrap_or(Duration::from_secs(DEFAULT_CHILD_DEADLINE_SECS))
}

/// Build a budget-exhaustion error naming the exhausted budget (DDL-1z1).
/// The message is what reaches stderr and the JSON error envelope, so the
/// budget name must stay in the text.
pub(crate) fn budget_exceeded(budget: &str, detail: String) -> DdlError {
    DdlError::InstallFailed(format!("budget exceeded: {budget} — {detail}"))
}

/// DDL-1z1 download budget for release assets, as a pure decision (no
/// network, no I/O): rejects a declared Content-Length over
/// [`MAX_RELEASE_ASSET_BYTES`] *before* any byte is fetched, and a streamed
/// byte count over the budget as chunks arrive (also covers the unknown-length
/// case, where the header cannot pre-check). Fail-closed: there is no
/// authorization that permits exceeding the budget.
pub fn enforce_download_budget(content_length: Option<u64>, streamed: u64) -> Result<()> {
    if let Some(len) = content_length
        && len > MAX_RELEASE_ASSET_BYTES
    {
        return Err(budget_exceeded(
            "download-size",
            format!(
                "release declares {len} bytes, exceeding the {MAX_RELEASE_ASSET_BYTES}-byte download budget"
            ),
        ));
    }
    if streamed > MAX_RELEASE_ASSET_BYTES {
        return Err(budget_exceeded(
            "download-size",
            format!(
                "release streamed {streamed} bytes with no declared length, exceeding the {MAX_RELEASE_ASSET_BYTES}-byte download budget"
            ),
        ));
    }
    Ok(())
}

/// Kill `child` and its descendants (DDL-1z1). The child was spawned as its
/// own process group leader, so a negative-pid SIGKILL sweeps the whole tree.
#[cfg(unix)]
pub(crate) fn kill_child_tree(child: &mut std::process::Child) {
    let pid = child.id() as libc::pid_t;
    if pid > 0 {
        // SAFETY: signal dispatch only; no memory is touched.
        unsafe {
            libc::kill(-pid, libc::SIGKILL);
        }
    }
    let _ = child.kill();
    let _ = child.wait();
}

/// Kill `child` on a platform without process-group signalling (DDL-1z1).
/// Only the direct child can be killed there; descendant sweeping is a
/// POSIX capability.
#[cfg(not(unix))]
pub(crate) fn kill_child_tree(child: &mut std::process::Child) {
    let _ = child.kill();
    let _ = child.wait();
}

/// Run a child to completion under the DDL-1z1 runtime budget: if the child
/// (and, on POSIX, its whole process group) has not exited within `deadline`,
/// it is killed and a budget-exhaustion error naming `child-runtime` is
/// returned. Stdio is whatever the caller configured.
pub fn run_child_bounded(
    cmd: &mut std::process::Command,
    deadline: std::time::Duration,
) -> Result<std::process::ExitStatus> {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    let mut child = cmd.spawn().map_err(DdlError::Io)?;
    let start = std::time::Instant::now();
    loop {
        if let Some(status) = child.try_wait().map_err(DdlError::Io)? {
            return Ok(status);
        }
        if start.elapsed() >= deadline {
            kill_child_tree(&mut child);
            return Err(budget_exceeded(
                "child-runtime",
                format!(
                    "child '{}' exceeded the {deadline:?} runtime budget and was killed (with its process group)",
                    cmd.get_program().display()
                ),
            ));
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

/// Run a child with piped stdio under both DDL-1z1 child budgets: the
/// runtime deadline (as [`run_child_bounded`]) and the captured-output cap
/// ([`MAX_CHILD_OUTPUT_BYTES`]). Output past the cap is discarded, the child
/// tree is killed, and the budget is named in the error — no silent
/// truncation.
/// Configure stdio (null stdin, piped out/err) and run the child in its own
/// process group so a deadline kill sweeps descendants too.
fn prepare_captured_command(
    cmd: &mut std::process::Command,
) -> std::io::Result<std::process::Child> {
    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    cmd.spawn()
}

/// Wait for a captured child, enforcing budgets: captured-output overflow
/// wins the race against a fast-exiting child (no silent truncation), the
/// runtime deadline kills hung children, and pipe EOF polls for the status
/// reap (process exit and reap are not atomic).
fn wait_captured_child(
    child: &mut std::process::Child,
    stdout_reader: &std::thread::JoinHandle<Vec<u8>>,
    stderr_reader: &std::thread::JoinHandle<Vec<u8>>,
    overflow: &std::sync::atomic::AtomicBool,
    deadline: std::time::Duration,
) -> Result<(Option<std::process::ExitStatus>, Option<&'static str>)> {
    use std::sync::atomic::Ordering;

    let start = std::time::Instant::now();
    let mut exhausted: Option<&'static str> = None;
    let mut status = None;
    loop {
        // Budget violations win the race against a fast-exiting child: a
        // child that floods past the cap is rejected even if it exits
        // cleanly a microsecond later (no silent truncation).
        if overflow.load(Ordering::Relaxed) {
            exhausted = Some("captured-output");
            break;
        }
        if let Some(s) = child.try_wait().map_err(DdlError::Io)? {
            status = Some(s);
            break;
        }
        if stdout_reader.is_finished() && stderr_reader.is_finished() {
            // Pipes hit EOF, so the child (and any pipe-holding descendant)
            // is gone; reap its status if it is ready. Poll briefly: process
            // exit and status reap are not atomic, and a missed reap here
            // would spuriously report "exited without a status".
            for _ in 0..100 {
                if let Ok(Some(s)) = child.try_wait() {
                    status = Some(s);
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            break;
        }
        if start.elapsed() >= deadline {
            exhausted = Some("child-runtime");
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    // Re-check after the loop: the child may have exited between the last
    // overflow sample and the reap.
    if exhausted.is_none() && overflow.load(Ordering::Relaxed) {
        exhausted = Some("captured-output");
    }
    Ok((status, exhausted))
}

/// Kill a budget-violating child and drain both readers (the group kill
/// closes every pipe end, so both readers reach EOF promptly — termination
/// stays within deadline + 1s).
fn kill_and_drain(
    child: &mut std::process::Child,
    stdout_reader: std::thread::JoinHandle<Vec<u8>>,
    stderr_reader: std::thread::JoinHandle<Vec<u8>>,
    budget: &'static str,
    cmd: &std::process::Command,
    deadline: std::time::Duration,
) -> DdlError {
    kill_child_tree(child);
    let _ = stdout_reader.join();
    let _ = stderr_reader.join();
    budget_exceeded(
        budget,
        format!(
            "child '{}' was killed (with its process group) for exceeding the {deadline:?} runtime budget",
            cmd.get_program().display()
        ),
    )
}

pub fn run_child_captured(
    cmd: &mut std::process::Command,
    deadline: std::time::Duration,
) -> Result<std::process::Output> {
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;

    let mut child = prepare_captured_command(cmd).map_err(DdlError::Io)?;

    let overflow = Arc::new(AtomicBool::new(false));
    let stdout_pipe = child.stdout.take();
    let stderr_pipe = child.stderr.take();
    let stdout_reader = drain_budgeted(stdout_pipe, Arc::clone(&overflow));
    let stderr_reader = drain_budgeted(stderr_pipe, Arc::clone(&overflow));

    let (status, exhausted) = wait_captured_child(
        &mut child,
        &stdout_reader,
        &stderr_reader,
        &overflow,
        deadline,
    )?;

    if let Some(budget) = exhausted {
        return Err(kill_and_drain(
            &mut child,
            stdout_reader,
            stderr_reader,
            budget,
            cmd,
            deadline,
        ));
    }

    let stdout = stdout_reader.join().unwrap_or_default();
    let stderr = stderr_reader.join().unwrap_or_default();
    Ok(std::process::Output {
        status: status
            .ok_or_else(|| DdlError::Other("child exited without a status".to_string()))?,
        stdout,
        stderr,
    })
}

/// Drain a child output pipe into a bounded buffer (DDL-1z1). Bytes past
/// [`MAX_CHILD_OUTPUT_BYTES`] raise `overflow` and are discarded so ddl's
/// memory stays bounded while the child is being killed.
pub(crate) fn drain_budgeted<R: std::io::Read + Send + 'static>(
    stream: Option<R>,
    overflow: std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut stream) = stream {
            let mut chunk = [0u8; 64 * 1024];
            loop {
                match stream.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if buf.len() + n > MAX_CHILD_OUTPUT_BYTES {
                            let keep = MAX_CHILD_OUTPUT_BYTES - buf.len();
                            buf.extend_from_slice(&chunk[..keep]);
                            overflow.store(true, std::sync::atomic::Ordering::Relaxed);
                        } else {
                            buf.extend_from_slice(&chunk[..n]);
                        }
                    }
                }
            }
        }
        buf
    })
}
