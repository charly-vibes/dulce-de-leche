//! Output formatting — human-readable and JSON envelope output.
//!
//! Every command supports both human-readable output (default) and JSON
//! output (via `--json`). JSON output follows the genesis-vibes envelope
//! format for machine-parsable responses.

use serde::Serialize;
use std::sync::Mutex;

use crate::error::{DdlError, Result};

/// Wrapper for genesis-vibes envelope types.
use genesis::envelope::{Envelope, EnvelopeKind, HintEntry, Warning};

/// Thread-local JSON output collector.
///
/// When active (via [`start_json_collection`]), JSON output functions push
/// their data here instead of printing a separate envelope. Callers can
/// retrieve the collected data with [`finish_json_collection`] and emit a
/// single envelope. Used by `cmd_init --json` to avoid multiple envelopes.
static JSON_COLLECTOR: Mutex<Option<Vec<serde_json::Value>>> = Mutex::new(None);

/// Begin collecting JSON output data instead of printing envelopes.
pub fn start_json_collection() {
    *JSON_COLLECTOR.lock().unwrap() = Some(Vec::new());
}

/// End collection and return the collected data, if any.
pub fn finish_json_collection() -> Option<Vec<serde_json::Value>> {
    JSON_COLLECTOR.lock().unwrap().take()
}

/// RAII guard that starts JSON collection on construction and emits a single
/// envelope with all collected data on drop (covers all return paths, including
/// early returns and errors).
pub struct JsonCollectorGuard {
    cli_version: &'static str,
    kind: EnvelopeKind,
}

impl JsonCollectorGuard {
    /// Start collecting JSON output. The guard emits a single envelope on drop.
    pub fn start(cli_version: &'static str, kind: EnvelopeKind) -> Self {
        start_json_collection();
        Self {
            cli_version,
            kind,
        }
    }
}

impl Drop for JsonCollectorGuard {
    fn drop(&mut self) {
        if let Some(results) = finish_json_collection() {
            let data = serde_json::json!({ "events": results });
            if let Ok(json_str) = json_output(
                self.cli_version,
                true,
                self.kind,
                data,
                vec![],
                vec![],
            ) {
                println!("{json_str}");
            }
        }
    }
}

/// Push a JSON value to the collector if active. Returns `true` if collected,
/// `false` if no collector is active (caller should print normally).
fn collect_json(data: &serde_json::Value) -> bool {
    let mut guard = JSON_COLLECTOR.lock().unwrap();
    if let Some(ref mut collector) = *guard {
        collector.push(data.clone());
        true
    } else {
        false
    }
}

/// Render a result as JSON using the genesis-vibes envelope format.
pub fn json_output<T: Serialize + std::fmt::Debug>(
    cli_version: &str,
    ok: bool,
    kind: EnvelopeKind,
    data: T,
    warnings: Vec<Warning>,
    hints: Vec<HintEntry>,
) -> Result<String> {
    if ok {
        let envelope = Envelope::success(cli_version, kind, data, warnings, hints);
        serde_json::to_string_pretty(&envelope)
            .map_err(|e| DdlError::Other(format!("JSON serialization failed: {e}")))
    } else {
        // For error cases, build an error envelope
        let err = genesis::envelope::ErrorResult::new(
            "DDL_ERR",
            &serde_json::to_string(&data).unwrap_or_else(|_| format!("{:?}", data)),
            None,
            None,
            None,
            vec![],
            vec![genesis::envelope::RemediationEntry {
                command: "ddl doctor".to_string(),
                description: "Run diagnostics for details".to_string(),
            }],
        )
        .map_err(|e| DdlError::Other(format!("Failed to build error envelope: {e}")))?;
        let envelope = Envelope::error(cli_version, err, warnings);
        serde_json::to_string_pretty(&envelope)
            .map_err(|e| DdlError::Other(format!("JSON serialization failed: {e}")))
    }
}

/// Print a success message with optional JSON fallback.
pub fn print_success(msg: &str, json: bool) {
    if json {
        let data = serde_json::json!({ "message": msg });
        // If a collector is active, buffer instead of printing separately.
        if collect_json(&data) {
            return;
        }
        if let Ok(json_str) = json_output(crate::VERSION, true, EnvelopeKind::Ok, data, vec![], vec![]) {
            println!("{json_str}");
        }
    } else {
        println!("{msg}");
    }
}

/// Print an error message with optional JSON fallback.
pub fn print_error(msg: &str, json: bool) {
    if json {
        let data = serde_json::json!({ "error": msg });
        if let Ok(json_str) = json_output(crate::VERSION, false, EnvelopeKind::Error, data, vec![], vec![]) {
            eprintln!("{json_str}");
        }
    } else {
        eprintln!("Error: {msg}");
    }
}

/// Print a formatted banner for ddl init.
pub fn print_banner(json: bool) {
    let version = crate::VERSION;
    if json {
        let data = serde_json::json!({
            "version": version,
            "name": "dulce-de-leche"
        });
        // If a collector is active, buffer instead of printing separately.
        if collect_json(&data) {
            return;
        }
        if let Ok(json_str) = json_output(crate::VERSION, true, EnvelopeKind::Info, data, vec![], vec![]) {
            println!("{json_str}");
        }
    } else {
        println!("╭──────────────────────────────────────╮");
        println!("│  dulce-de-leche — charly-vibes       │");
        println!("│  bundle orchestrator v{version}       │");
        println!("╰──────────────────────────────────────╯");
        println!();
    }
}

/// Print a tool installation result line.
pub fn print_install_result(success: bool, tool: &str, message: &str, json: bool) {
    if json {
        let data = serde_json::json!({
            "tool": tool,
            "success": success,
            "message": message
        });
        // If a collector is active, buffer instead of printing separately.
        if collect_json(&data) {
            return;
        }
        if let Ok(json_str) = json_output(
            crate::VERSION,
            success,
            if success {
                EnvelopeKind::Ok
            } else {
                EnvelopeKind::Warning
            },
            data,
            vec![],
            vec![],
        ) {
            println!("{json_str}");
        }
    } else {
        let icon = if success { "✓" } else { "✗" };
        println!("  {icon} {message}");
    }
}
