//! dulce-de-leche (ddl) — orchestrator for the charly-vibes tool ecosystem.
//!
//! One binary, one command, any platform. Installs, configures, and updates
//! every charly-vibes tool from a single CLI.

pub mod catalog;
pub mod cli;
pub mod commands;
pub mod compat;
pub mod contract_bundle;
pub mod diagnostics;
pub mod dot_ddl;
pub mod error;
pub mod gates;
pub mod installer;
pub mod manifest;
pub mod output;
pub mod platform;
pub mod repo_status;
pub mod setup_wizard;
pub mod skills;

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

/// START marker of ddl's own managed block in AGENTS.md, as written by
/// [`agents_block`] (genesis::aix `<!-- NAME:START -->` format). Shared
/// constant so the writer (init) and the checker (ddl.repo.blocks doctor
/// check, DDL-fw7) can never drift apart.
pub const DDL_AGENTS_MARKER: &str = "<!-- DDL:START -->";

/// START marker of the session-close protocol block (DDL-9kz.1). Same
/// genesis::aix `<!-- NAME:START -->` wrapping as [`DDL_AGENTS_MARKER`];
/// shared so the writer (gates wiring), the checker (ddl.repo.blocks), and
/// the block text can never drift apart unnoticed.
pub const SESSION_PROTOCOL_MARKER: &str = "<!-- SESSION-PROTOCOL:START -->";

/// Generate the harness-agnostic session-close ritual block (DDL-9kz.1).
///
/// pi traverses to the global `~/AGENTS.md` close protocol; harnesses that
/// stop at the repo root do repo work and never close it out. This block
/// carries the ritual verbatim — including the exact bead-export line,
/// because agents copy command lines literally — so every harness sees it
/// at the repo root. It is not harness-specific (no harness names) and does
/// not duplicate the wai managed block (which stays the wai reference);
/// the wai `close` step is named once as part of the ritual.
pub fn session_protocol_block() -> String {
    genesis::aix::agents_block(
        "SESSION-PROTOCOL",
        r#"
## Session-close protocol (every harness, every session)

A work session is NOT complete when the edits are done — it is complete
when the tracker and git reflect it. Close out with this ritual:

1. File issues for remaining work: `bd create <title>` (bd owns issue
   lifecycle; do not leave follow-ups in chat or TODO comments).
2. Run the quality gates: `just ci` (tests/linters/build; red -> green
   before you commit).
3. Close finished work with evidence: `bd close <id> -r "<what shipped,
   how verified>"`.
4. Export the tracker (git is the sync mechanism; the global gitignore
   excludes `.beads/`, hence the forced add):
   `bd export -o .beads/issues.jsonl && git add -f .beads/issues.jsonl && git commit`
5. `git pull --rebase` then `git push`. Work is NOT complete until the
   push succeeds; never say "ready to push when you are" — push.
6. `wai close` so the next session can resume cleanly.
"#,
    )
}

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
