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

use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};
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

        /// Wire the end state users actually ask for: lefthook hard gates
        /// (pre-commit: ah check, pretender gate, spk lint; pre-push: ah
        /// check), tool-data .gitignore entries, beads no-db stamping, and
        /// openspec-init-before-ah ordering. Idempotent — safe to re-run.
        #[arg(long)]
        gates: bool,
    },

    /// Install a single tool by name
    Install {
        /// Name of the tool to install (all managed tools are listed below;
        /// crate-name aliases like 'espectacular' also resolve)
        tool: String,
    },

    /// Show the charly-vibes tool catalog — what exists, what it does,
    /// how to invoke it
    Catalog,

    /// Submit a feedback issue to the ddl repository
    Feedback {
        /// Kind of issue (bug|feature|question|chore)
        kind: String,

        /// Auto-populate the body from the last failed ddl command
        #[arg(long)]
        from_last_error: bool,

        /// Print the issue body and gh command without submitting
        #[arg(long)]
        dry_run: bool,

        /// Override the issue title (wins over derived titles)
        #[arg(long)]
        title: Option<String>,
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
    /// Comma-separated canonical tool names from MANAGED_TOOLS — the single
    /// source of truth for help examples (DDL-6zn.8). Hardcoded example
    /// lists drifted twice (fotos-mcp/fabbro stale, vampiro/specodelic
    /// missing); generated lines cannot drift.
    pub fn managed_tools_line() -> String {
        let names: Vec<&str> = crate::platform::MANAGED_TOOLS
            .iter()
            .map(|t| t.name)
            .collect();
        names.join(", ")
    }

    /// Parse CLI args and return the parsed structure.
    ///
    /// Builds the command so the install/init help text is GENERATED from
    /// MANAGED_TOOLS instead of hand-maintained doc comments.
    pub fn parse_or_exit() -> Self {
        let mut cmd = Args::command();
        let tools_line = Self::managed_tools_line();
        cmd = cmd.mut_subcommand("install", |c| {
            c.after_help(format!(
                "Managed tools: {tools_line}\n\
                 Crate-name aliases resolve to their tool (e.g. \
                 'espectacular' → ah). Run `ddl catalog` for descriptions \
                 and install methods."
            ))
        });
        cmd = cmd.mut_subcommand("init", |c| {
            c.after_help(format!(
                "--tools accepts a comma-separated subset (default: all): {tools_line}"
            ))
        });
        let matches = cmd.get_matches();
        Args::from_arg_matches(&matches).unwrap_or_else(|e| e.exit())
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
