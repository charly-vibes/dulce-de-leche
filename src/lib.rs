//! dulce-de-leche (ddl) — orchestrator for the charly-vibes tool ecosystem.
//!
//! One binary, one command, any platform. Installs, configures, and updates
//! every charly-vibes tool from a single CLI.

pub mod cli;
pub mod diagnostics;
pub mod dot_ddl;
pub mod error;
pub mod installer;
pub mod manifest;
pub mod output;
pub mod platform;

use std::path::PathBuf;

/// Find the nearest `.ddl/` directory by walking up from CWD.
pub fn find_ddl_dir() -> Option<PathBuf> {
    let mut cwd = std::env::current_dir().ok()?;
    loop {
        let candidate = cwd.join(".ddl");
        if candidate.is_dir() {
            return Some(candidate);
        }
        if !cwd.pop() {
            return None;
        }
    }
}

/// The version of ddl embedded at build time.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");