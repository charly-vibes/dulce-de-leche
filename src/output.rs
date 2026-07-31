//! Output formatting — human-readable and JSON envelope output.
//!
//! Every command supports both human-readable output (default) and JSON
//! output (via `--json`). JSON output follows the genesis-vibes envelope
//! format for machine-parsable responses.

use serde::Serialize;

use crate::error::{DdlError, Result};

/// Wrapper for genesis-vibes envelope types.
use genesis::envelope::{Envelope, EnvelopeKind, HintEntry, Warning};

/// Render a result as JSON using the genesis-vibes envelope format.
pub fn json_output<T: Serialize + std::fmt::Debug>(
    ok: bool,
    kind: EnvelopeKind,
    data: T,
    warnings: Vec<Warning>,
    hints: Vec<HintEntry>,
) -> Result<String> {
    if ok {
        let envelope = Envelope::success(kind, data, warnings, hints);
        serde_json::to_string_pretty(&envelope)
            .map_err(|e| DdlError::Other(format!("JSON serialization failed: {e}")))
    } else {
        // For error cases, build an error envelope
        let err = genesis::envelope::ErrorResult::new(
            "DDL_ERR",
            &format!("{:?}", data),
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
        let envelope = Envelope::error(err, warnings);
        serde_json::to_string_pretty(&envelope)
            .map_err(|e| DdlError::Other(format!("JSON serialization failed: {e}")))
    }
}

/// Print a success message with optional JSON fallback.
pub fn print_success(msg: &str, json: bool) {
    if json {
        let data = serde_json::json!({ "message": msg });
        if let Ok(json_str) = json_output(true, EnvelopeKind::Ok, data, vec![], vec![]) {
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
        if let Ok(json_str) = json_output(false, EnvelopeKind::Error, data, vec![], vec![]) {
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
        if let Ok(json_str) = json_output(true, EnvelopeKind::Info, data, vec![], vec![]) {
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
        if let Ok(json_str) = json_output(
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
