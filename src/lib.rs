//! dulce-de-leche (ddl) — orchestrator for the charly-vibes tool ecosystem.
//!
//! One binary, one command, any platform. Installs, configures, and updates
//! every charly-vibes tool from a single CLI.

pub mod cli;
pub mod compat;
pub mod diagnostics;
pub mod dot_ddl;
pub mod error;
pub mod installer;
pub mod manifest;
pub mod output;
pub mod platform;

// Re-export DdlConfig for use in tests and by downstream consumers.
pub use dot_ddl::DdlConfig;

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

/// Generate ddl's agents block for AIX artifacts (llm.txt, AGENTS.md).
///
/// Uses genesis::aix::agents_block for consistent marker wrapping.
pub fn agents_block() -> String {
    genesis::aix::agents_block(
        "DDL",
        &format!(
            r#"
## dulce-de-leche (ddl v{version})

Orchestrator for the charly-vibes tool ecosystem. Run `ddl init` to
install and configure all tools. Run `ddl status` for a health overview.

Commands:
- init     Bootstrap the toolset
- install  Install a single tool
- status   Show health overview
- doctor   Run diagnostics
- version  Show versions
- upgrade  Update tools
- migrate  Migrate configs
- scope    Show active .ddl/
"#,
            version = VERSION
        ),
    )
}
