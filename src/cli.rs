//! CLI command structure using clap derive.

use clap::{Parser, Subcommand};

/// dulce-de-leche (ddl) — orchestrate the charly-vibes tool ecosystem.
///
/// One command to install, configure, and update every charly-vibes tool.
#[derive(Parser, Debug)]
#[command(name = "ddl", version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Args {
    /// Enable verbose output
    #[arg(short = 'v', long = "verbose", global = true)]
    pub verbose: bool,

    /// Suppress output except errors
    #[arg(short = 'q', long = "quiet", global = true)]
    pub quiet: bool,

    /// Non-interactive mode — use defaults for all prompts
    #[arg(short = 'y', long = "yes", global = true)]
    pub yes: bool,

    /// Output as JSON for machine parsing
    #[arg(long = "json", global = true)]
    pub json: bool,

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
}

impl Args {
    /// Parse CLI args and return the parsed structure.
    pub fn parse_or_exit() -> Self {
        Self::parse()
    }
}