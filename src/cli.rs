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

    /// Guided first-time setup — walk from zero to a verified-initialized
    /// repo via the wizard step machine (pre-flight → migrate → scope →
    /// delegate → verify, backed by the exact `ddl init` code paths)
    Setup {
        /// Comma-separated list of tools to install (default: all)
        #[arg(long, value_name = "TOOLS")]
        tools: Option<String>,

        /// Skip tool installation, only configure existing tools
        #[arg(long)]
        no_install: bool,
    },

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
        // DDL-uu8 (SEC-04): the execution scopes of the install plan are
        // disclosed in the help text so their authorization requirements are
        // discoverable before running.
        let scopes_note = "Execution scopes (restricted-privilege policy): children run with \
                           an allowlisted environment; set \
                           DDL_ALLOW_UNRESTRICTED_CHILD_ENV=1 to restore the legacy \
                           inherited environment. Global skill installs write outside \
                           this project and require DDL_ALLOW_GLOBAL_SKILL=1."
            .to_string();
        cmd = cmd.mut_subcommand("install", |c| {
            c.after_help(format!(
                "Managed tools: {tools_line}\n\
                 Crate-name aliases resolve to their tool (e.g. \
                 'espectacular' → ah). Run `ddl catalog` for descriptions \
                 and install methods.\n\n{scopes_note}"
            ))
        });
        cmd = cmd.mut_subcommand("init", |c| {
            c.after_help(format!(
                "--tools accepts a comma-separated subset (default: all): {tools_line}\
                 \n\n{scopes_note}"
            ))
        });
        let matches = cmd.get_matches();
        Args::from_arg_matches(&matches).unwrap_or_else(|e| e.exit())
    }

    /// Convenience: is JSON output requested or auto-detected?
    pub fn is_json(&self) -> bool {
        self.format.is_json()
    }

    /// Comma-separated, trimmed, non-empty tool names from a `--tools`
    /// value (shared shape with `Init::tools` parsing).
    fn parse_tools(tools: Option<&str>) -> Vec<String> {
        tools
            .map(|raw| {
                raw.split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// SetupWizard entry point (openspec add-setup-wizard-onboarding §2.2).
    ///
    /// Runs the five-step machine from `src/setup_wizard.rs` (pre-flight →
    /// migrate? → scope → delegate to init/install paths → verify) through
    /// the production [`crate::setup_wizard::InitPathsDelegate`] — the same
    /// code paths `ddl init` uses, never a fork. The JSON envelope carries
    /// the per-step outcomes plus the machine verdict and resume summary,
    /// still identifying itself via `data.command == "setup"` with the
    /// flag passthrough (pinned by tests/setup_command.rs). Step failures
    /// keep init's exit-non-zero contract.
    pub fn cmd_setup(
        tools: Option<String>,
        no_install: bool,
        yes: bool,
        args: &Args,
    ) -> crate::error::Result<()> {
        use crate::setup_wizard::{InitPathsDelegate, SetupWizard, WizardConfig};

        let platform = crate::platform::Platform::detect().ok_or_else(|| {
            crate::error::DdlError::UnsupportedPlatform("Could not detect platform".to_string())
        })?;

        let parsed_tools = Self::parse_tools(tools.as_deref());
        let verbose = args.verbose_quiet.raw_count() >= 1;
        // Same non-interactive rule as cmd_init's init cascade: -y or JSON.
        let non_interactive = yes || args.is_json();

        let ddl_dir = crate::dot_ddl::DdlDir::find_or_create()?;
        let mut delegate =
            InitPathsDelegate::new(ddl_dir, platform, no_install, verbose, non_interactive);
        let config = WizardConfig {
            tools: parsed_tools.clone(),
            no_install,
            yes,
        };
        let report = SetupWizard::new(config, &mut delegate).run();

        if args.is_json() {
            let steps: Vec<serde_json::Value> = report
                .steps
                .iter()
                .map(|s| {
                    serde_json::json!({
                        "step": s.step.label(),
                        "status": s.status,
                        "failures": s.failures,
                        "detail": s.detail,
                    })
                })
                .collect();
            let data = serde_json::json!({
                "command": "setup",
                "tools": parsed_tools,
                "no_install": no_install,
                "yes": yes,
                "steps": steps,
                "ok": report.ok,
                "resume_summary": report.resume_summary,
            });
            // Envelope-level ok stays true: the command ran and reported.
            // The machine verdict lives in data.ok; the exit code carries
            // failure (below) and main routes the error envelope to stderr,
            // so stdout keeps exactly one JSON document.
            let json_str = crate::output::json_output(
                crate::VERSION,
                true,
                genesis::envelope::EnvelopeKind::Info,
                data,
                vec![],
                vec![],
            )?;
            println!("{json_str}");
        } else {
            println!("ddl setup — guided setup wizard");
            for step in &report.steps {
                if step.status == "ok" {
                    println!("  ✓ {}", step.step.label());
                } else {
                    println!("  ✗ {}", step.step.label());
                    for f in &step.failures {
                        println!("      {} — {}", f.subject, f.message);
                    }
                }
            }
            if !report.resume_summary.is_empty() {
                println!();
                println!("Setup did not finish — resume manually or re-run:");
                for line in &report.resume_summary {
                    println!("{line}");
                }
            }
        }

        if !report.ok {
            return Err(crate::error::DdlError::PartialFailure);
        }
        Ok(())
    }

    /// Convenience: is human output requested or auto-detected?
    pub fn is_human(&self) -> bool {
        self.format.is_human()
    }
}
