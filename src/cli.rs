//! CLI command structure using clap derive.
//!
//! Uses genesis::guide for verbosity and output format:
//!
//! **Verbosity:** `-v`/`-vv`/`-vvv` for progressive-disclosure output (was
//! a single `--verbose` bool before the genesis adoption). `-q` for silence.
//!
//! **Output format:** `--json` / `--human` / auto-detect.
//! When neither `--json` nor `--human` is passed, the format is auto-detected:
//! human-readable for TTYs, JSON envelopes for piped/redirected stdout.
//! This means agents and CI pipelines get machine-readable output by default.
//! Use `--human` to force human output in a non-TTY context.
//!
//! The `Completions` subcommand ignores the format flags — completions are
//! always plain shell script text.

use clap::{Parser, Subcommand};
use genesis::guide::{CliFormat, CliVerbosity};

/// dulce-de-leche (ddl) — orchestrate the charly-vibes tool ecosystem.
///
/// One command to install, configure, and update every charly-vibes tool.
#[derive(Parser, Debug)]
#[command(name = "ddl", version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Args {
    #[command(flatten)]
    pub verbose_quiet: CliVerbosity,

    #[command(flatten)]
    pub format: CliFormat,

    /// Non-interactive mode — use defaults for all prompts
    #[arg(short = 'y', long = "yes", global = true)]
    pub yes: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Bootstrap the charly-vibes toolset — install, configure, and init
    Init {
        /// Comma-separated list of tools to install (default: all)
        #[arg(long, value_name = "TOOLS")]
        tools: Option<String>,

        /// Skip tool installation, only configure existing tools
        #[arg(long)]
        no_install: bool,
    },

    /// Install a single tool by name
    Install {
        /// Name of the tool to install (e.g., wai, dont, ah, pretender,
        /// testaruda, fotos-mcp, fabbro)
        tool: String,
    },

    /// Show cross-tool health overview
    Status,

    /// Run detailed diagnostics across all tools
    Doctor {
        /// Attempt to auto-fix detected issues
        #[arg(long)]
        fix: bool,
    },

    /// Show versions of ddl and all managed tools
    Version {
        /// Check latest available versions (requires network)
        #[arg(long)]
        check: bool,
    },

    /// Update all tools to latest compatible versions
    Upgrade {
        /// Optional: upgrade a specific tool only
        tool: Option<String>,
    },

    /// Migrate existing configs under .ddl/
    Migrate {
        /// Undo migration — restore previous layout
        #[arg(long)]
        undo: bool,
    },

    /// Show which .ddl/ is active
    Scope,

    /// Generate shell completions
    Completions {
        /// The shell to generate completions for
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
}

impl Args {
    /// Parse CLI args and return the parsed structure.
    pub fn parse_or_exit() -> Self {
        Self::parse()
    }

    /// Convenience: is JSON output requested or auto-detected?
    pub fn is_json(&self) -> bool {
        self.format.is_json()
    }

    /// Convenience: is human output requested or auto-detected?
    pub fn is_human(&self) -> bool {
        self.format.is_human()
    }
}
