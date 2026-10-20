//! dulce-de-leche (ddl) — setup wizard step machine.
//!
//! **Purpose:** orchestrate guided first-time setup as five ordered steps —
//! pre-flight → migrate? → scope → delegate → verify — without owning any
//! of the work itself (openspec add-setup-wizard-onboarding §2.2).
//!
//! **Responsibilities:**
//! - Define the canonical [`WizardStep`] order and drive a run through it.
//! - Collect per-step outcomes and fold failures into a resume summary
//!   with the same exit-non-zero contract as `ddl init`.
//! - Forward wizard-level flags (`--yes`, `--tools`, `--no-install`) into
//!   the step seams instead of re-interpreting them ad hoc.
//!
//! **Rationale:** the wizard is an orchestrator, never a reimplementation.
//! Every real action flows through the [`WizardDelegate`] seams; the
//! production delegate (wired in §2.3) backs them with the exact code
//! paths `ddl init` uses — `installer::check_prerequisites` for pre-flight,
//! the migrate dry-run report, `installer::install_tool` +
//! `installer::run_tool_init` for the cascade, and `ddl status` /
//! `ddl doctor` checks for verify. Duplicating any of that here would
//! fork the init contract; this module only sequences and reports.
//!
//! §2.3 wired this module into the library (`pub mod setup_wizard;` in
//! lib.rs) and backed the seams with the init/install paths via
//! [`InitPathsDelegate`].

use serde::Serialize;

use crate::dot_ddl::DdlDir;
use crate::installer::InstallMethod;
use crate::platform::Platform;

/// The five wizard steps, in mandatory execution order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum WizardStep {
    /// Platform + prerequisite checks (same checks as `ddl init`),
    /// surfaced as a list with human-readable fixes.
    PreFlight,
    /// Fresh-config vs migrate-existing decision (delegates to the
    /// migrate dry-run report before anything is applied).
    Migrate,
    /// Full fleet vs category/tool subset (registry categories are the
    /// taxonomy).
    Scope,
    /// Run the install + tool-init cascade — the same code paths
    /// `ddl init` uses.
    Delegate,
    /// End with the `ddl status` / `ddl doctor` verification pass.
    Verify,
}

impl WizardStep {
    /// Canonical order; a run executes every step exactly once, in this
    /// order. Pinned by tests/setup_wizard.rs.
    pub const ALL: [WizardStep; 5] = [
        WizardStep::PreFlight,
        WizardStep::Migrate,
        WizardStep::Scope,
        WizardStep::Delegate,
        WizardStep::Verify,
    ];

    /// Human-readable step name (used in reports and resume summaries).
    pub fn label(self) -> &'static str {
        match self {
            WizardStep::PreFlight => "pre-flight",
            WizardStep::Migrate => "migrate",
            WizardStep::Scope => "scope",
            WizardStep::Delegate => "delegate",
            WizardStep::Verify => "verify",
        }
    }
}

/// One pre-flight prerequisite check with a human-readable fix hint when
/// it fails (design.md step 1: "surfaced as a pre-flight list with
/// human-readable fixes").
#[derive(Debug, Clone, Serialize)]
pub struct PreFlightCheck {
    /// What was checked, e.g. `"curl"` or `"rustc"`.
    pub name: String,
    /// Whether the prerequisite is satisfied.
    pub ok: bool,
    /// Human-readable fix for a failed check; empty when `ok`.
    pub fix: String,
}

/// The fresh-vs-migrate decision, delegated to the migrate dry-run report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum MigrateDecision {
    /// Start with a fresh `.ddl/` config.
    Fresh,
    /// Migrate existing configs; carries the dry-run report lines that
    /// were shown before applying.
    Migrate {
        /// Dry-run report lines (what would move, per `ddl migrate --dry-run`).
        dry_run: Vec<String>,
    },
}

/// What the cascade should cover. Registry categories (core /
/// recommended / extension) are the taxonomy; an explicit `--tools` list
/// narrows it to specific tools.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum ScopeSelection {
    /// Everything the registry manages.
    All,
    /// A concrete subset — category names, tool names, or both.
    Categories(Vec<String>),
}

/// A single failure from any step; folds into the resume summary.
#[derive(Debug, Clone, Serialize)]
pub struct StepFailure {
    /// Which step produced the failure.
    pub step: WizardStep,
    /// What failed, e.g. a tool name or a check name.
    pub subject: String,
    /// Human-readable explanation (same style as init failures).
    pub message: String,
}

/// Wizard-level flag passthrough, mirroring `ddl setup`'s CLI surface
/// (pinned by tests/setup_command.rs): `--tools`, `--no-install`, `-y`.
#[derive(Debug, Clone, Default)]
pub struct WizardConfig {
    /// Explicit tool list from `--tools` (already split/trimmed).
    pub tools: Vec<String>,
    /// Skip tool installation, only configure existing tools.
    pub no_install: bool,
    /// Non-interactive mode — delegates skip their prompts.
    pub yes: bool,
}

/// The delegation seams. One method per action the wizard must not
/// implement itself; the §2.3 production delegate backs each with the
/// init/install code paths.
pub trait WizardDelegate {
    /// Step 1 — prerequisite checks (same checks as `ddl init`).
    fn pre_flight(&self) -> Vec<PreFlightCheck>;

    /// Step 2 — the migrate? prompt. `non_interactive` is `true` under
    /// `-y`: the delegate must return its default without prompting.
    fn migrate_decision(&self, non_interactive: bool) -> MigrateDecision;

    /// Step 3 — the scope choice (full fleet vs category subset).
    fn scope_selection(&self) -> ScopeSelection;

    /// Step 4 — the install + tool-init cascade over `scope`. Honors the
    /// wizard config (`--no-install`, `--tools`) it was constructed with.
    fn run_cascade(&mut self, scope: &ScopeSelection) -> Vec<StepFailure>;

    /// Step 5 — verify via the `ddl status` / `ddl doctor` checks.
    fn verify(&self) -> Vec<StepFailure>;
}

/// One executed step and what came out of it.
#[derive(Debug, Clone, Serialize)]
pub struct StepOutcome {
    /// Which step ran.
    pub step: WizardStep,
    /// `"ok"` or `"failed"` — failed means at least one [`StepFailure`].
    pub status: &'static str,
    /// Failures recorded by this step (empty when ok).
    pub failures: Vec<StepFailure>,
    /// Step-specific structured detail (pre-flight list, migrate
    /// decision, scope selection) for machine consumers.
    pub detail: serde_json::Value,
}

/// The full result of a wizard run.
#[derive(Debug, Clone, Serialize)]
pub struct WizardReport {
    /// One outcome per executed step, in [`WizardStep::ALL`] order.
    pub steps: Vec<StepOutcome>,
    /// `true` only when every step reported ok.
    pub ok: bool,
    /// Resume summary lines — one per failure, naming subject and fix —
    /// so a re-run (or a human) knows exactly where to pick up. Same
    /// contract as init's resume summary.
    pub resume_summary: Vec<String>,
}

/// The step machine: holds the flag passthrough and a delegate, and
/// sequences the five steps. It decides nothing and does nothing itself.
pub struct SetupWizard<'a> {
    config: WizardConfig,
    delegate: &'a mut dyn WizardDelegate,
}

impl std::fmt::Debug for SetupWizard<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SetupWizard")
            .field("config", &self.config)
            .field("delegate", &"dyn WizardDelegate")
            .finish()
    }
}

impl<'a> SetupWizard<'a> {
    /// Bind a config (flag passthrough) and a delegate to drive.
    pub fn new(config: WizardConfig, delegate: &'a mut dyn WizardDelegate) -> Self {
        Self { config, delegate }
    }

    /// Run all five steps in order and fold the outcomes into a report.
    ///
    /// Steps never short-circuit: a pre-flight or cascade failure is
    /// recorded, later steps still run, and [`WizardReport::ok`] plus the
    /// resume summary carry the verdict — matching init's
    /// "exit non-zero with a resume summary" contract.
    pub fn run(self) -> WizardReport {
        let mut steps = Vec::with_capacity(WizardStep::ALL.len());

        // Step 1 — pre-flight.
        let checks = self.delegate.pre_flight();
        let pre_failures: Vec<StepFailure> = checks
            .iter()
            .filter(|c| !c.ok)
            .map(|c| StepFailure {
                step: WizardStep::PreFlight,
                subject: c.name.clone(),
                message: if c.fix.is_empty() {
                    "prerequisite missing".to_string()
                } else {
                    c.fix.clone()
                },
            })
            .collect();
        steps.push(StepOutcome {
            step: WizardStep::PreFlight,
            status: Self::status_of(&pre_failures),
            failures: pre_failures,
            detail: serde_json::to_value(&checks).unwrap_or(serde_json::Value::Null),
        });

        // Step 2 — migrate? (delegate owns the prompt; -y forwards
        // non-interactive so the delegate skips it).
        let migrate = self.delegate.migrate_decision(self.config.yes);
        steps.push(StepOutcome {
            step: WizardStep::Migrate,
            status: "ok",
            failures: vec![],
            detail: serde_json::to_value(&migrate).unwrap_or(serde_json::Value::Null),
        });

        // Step 3 — scope. An explicit --tools list narrows whatever the
        // delegate chose; otherwise the delegate's selection stands.
        let scope = if self.config.tools.is_empty() {
            self.delegate.scope_selection()
        } else {
            ScopeSelection::Categories(self.config.tools.clone())
        };
        steps.push(StepOutcome {
            step: WizardStep::Scope,
            status: "ok",
            failures: vec![],
            detail: serde_json::to_value(&scope).unwrap_or(serde_json::Value::Null),
        });

        // Step 4 — delegate the install + tool-init cascade.
        let cascade_failures = self.delegate.run_cascade(&scope);
        steps.push(StepOutcome {
            step: WizardStep::Delegate,
            status: Self::status_of(&cascade_failures),
            failures: cascade_failures.clone(),
            detail: serde_json::json!({
                "scope": scope,
                "no_install": self.config.no_install,
            }),
        });

        // Step 5 — verify. Always runs, even after a cascade failure.
        let verify_failures = self.delegate.verify();
        steps.push(StepOutcome {
            step: WizardStep::Verify,
            status: Self::status_of(&verify_failures),
            failures: verify_failures.clone(),
            detail: serde_json::Value::Null,
        });

        // §2.4 parity: delegate-step failures are the wizard's analog of
        // init-cascade failures, so they render through init's OWN summary
        // builder (`installer::resume_summary`) — the wizard never forks
        // the init resume-summary format. Other steps' failures (pre-flight,
        // verify) have no init analog and keep the wizard-local format.
        let mut resume_summary = Vec::new();
        for s in &steps {
            for f in &s.failures {
                if f.step == WizardStep::Delegate {
                    resume_summary.extend(crate::installer::resume_summary(&[
                        crate::installer::InitFailure {
                            tool: f.subject.clone(),
                            detail: f.message.clone(),
                        },
                    ]));
                } else {
                    resume_summary.push(format!(
                        "{}: {} — {}",
                        f.step.label(),
                        f.subject,
                        f.message
                    ));
                }
            }
        }

        let ok = steps.iter().all(|s| s.status == "ok");
        WizardReport {
            steps,
            ok,
            resume_summary,
        }
    }

    fn status_of(failures: &[StepFailure]) -> &'static str {
        if failures.is_empty() { "ok" } else { "failed" }
    }
}

// ── Failure-output parity (§2.4) ──────────────────────────────────

/// Emit delegate-step failures exactly the way `ddl init` emits
/// init-cascade failures in JSON mode: through
/// [`crate::output::print_install_result`] with `success = false` —
/// the same `{tool, success, message}` event keys and the same Warning
/// envelope level. The wizard never forks the init failure serialization
/// (parity pinned by tests/setup_wizard.rs).
pub fn emit_failures_like_init(failures: &[StepFailure]) {
    for f in failures.iter().filter(|f| f.step == WizardStep::Delegate) {
        crate::output::print_install_result(false, &f.subject, &f.message, true);
    }
}

// ── Production delegate (§2.3 reuse contract) ─────────────────────────

/// The production delegate: every seam is backed by the exact code path
/// `ddl init` uses — never a fork.
///
/// - pre-flight: the prerequisite checks init's install paths perform,
///   derived from [`crate::installer::best_install_method`] +
///   [`InstallMethod::check_prerequisites`] (never a hardcoded list).
/// - migrate: the same legacy-config scan `ddl migrate` uses
///   ([`DdlDir::detect_legacy_configs`]) as the dry-run report.
/// - cascade: [`crate::installer::install_selected_tools`] +
///   [`crate::installer::run_tool_init`], prerequisite-ordered via
///   [`crate::gates::tool_init_order`], with the same manifest
///   bookkeeping (`record_installed`/`record_failed`/`record_inited`).
/// - verify: the same status report `ddl status` builds
///   ([`crate::diagnostics::build_status_report`]).
pub struct InitPathsDelegate {
    ddl_dir: DdlDir,
    platform: Platform,
    no_install: bool,
    verbose: bool,
    non_interactive: bool,
}

impl InitPathsDelegate {
    /// Bind the delegate to a `.ddl/` dir, the detected platform, and the
    /// wizard flag passthrough (`--no-install`) plus verbosity/interactive
    /// mode, mirroring what `cmd_init` forwards into the same paths.
    pub fn new(
        ddl_dir: DdlDir,
        platform: Platform,
        no_install: bool,
        verbose: bool,
        non_interactive: bool,
    ) -> Self {
        Self {
            ddl_dir,
            platform,
            no_install,
            verbose,
            non_interactive,
        }
    }
}

impl WizardDelegate for InitPathsDelegate {
    fn pre_flight(&self) -> Vec<PreFlightCheck> {
        // The union of install methods the managed fleet requires — the
        // same selection init makes per tool — each checked with the
        // installer's own prerequisite check.
        let mut seen: Vec<InstallMethod> = Vec::new();
        let mut checks = Vec::new();
        for tool in crate::platform::MANAGED_TOOLS {
            let method = crate::installer::best_install_method(tool, &self.platform);
            if seen.contains(&method) {
                continue;
            }
            seen.push(method.clone());
            let name = method.to_string();
            match method.check_prerequisites() {
                Ok(()) => checks.push(PreFlightCheck {
                    name,
                    ok: true,
                    fix: String::new(),
                }),
                Err(e) => checks.push(PreFlightCheck {
                    name,
                    ok: false,
                    fix: e.to_string(),
                }),
            }
        }
        checks
    }

    fn migrate_decision(&self, _non_interactive: bool) -> MigrateDecision {
        // The migrate dry-run report: the same legacy-config scan
        // `ddl migrate` performs, surfaced as would-move lines before
        // anything is applied.
        let legacy = self.ddl_dir.detect_legacy_configs();
        if legacy.is_empty() {
            MigrateDecision::Fresh
        } else {
            MigrateDecision::Migrate {
                dry_run: legacy
                    .iter()
                    .map(|(tool, path)| format!("would move {} → .ddl/{tool}/", path.display()))
                    .collect(),
            }
        }
    }

    fn scope_selection(&self) -> ScopeSelection {
        // Default scope: the full fleet — init's default when no --tools.
        ScopeSelection::All
    }

    fn run_cascade(&mut self, scope: &ScopeSelection) -> Vec<StepFailure> {
        // Exactly the install + tool-init cascade `ddl init` runs.
        let tools: Vec<String> = match scope {
            ScopeSelection::All => crate::platform::MANAGED_TOOLS
                .iter()
                .map(|t| t.name.to_string())
                .collect(),
            ScopeSelection::Categories(names) => names.clone(),
        };

        let mut failures = Vec::new();
        let mut init_candidates: Vec<String> = Vec::new();

        if self.no_install {
            // Configure-only mode still runs the init cascade for the
            // selected tools; absent tools are skipped gracefully by
            // run_tool_init's PATH/dest probe (same as init --no-install).
            init_candidates = tools
                .iter()
                .filter(|t| t.as_str() != "wai")
                .cloned()
                .collect();
        } else if !tools.is_empty() {
            let results = crate::installer::install_selected_tools(
                &tools,
                &self.platform,
                &mut self.ddl_dir.manifest,
                self.verbose,
            );
            for result in &results {
                if result.success {
                    if let Err(e) = self.ddl_dir.record_installed(
                        result.tool,
                        &result.version,
                        &result.method.to_string(),
                    ) {
                        eprintln!(
                            "  ⚠ could not record install for {result_tool}: {e}",
                            result_tool = result.tool
                        );
                    }
                    // wai is force-initialized after the cascade, same as init.
                    if result.tool != "wai" {
                        init_candidates.push(result.tool.to_string());
                    }
                } else {
                    let _ = self
                        .ddl_dir
                        .record_failed(result.tool, &result.method.to_string());
                    failures.push(StepFailure {
                        step: WizardStep::Delegate,
                        subject: result.tool.to_string(),
                        message: result.message.clone(),
                    });
                }
            }
        }

        // Prerequisite-ordered init cascade — same order init uses.
        let ordered = crate::gates::tool_init_order(
            &init_candidates
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
        );
        for tool_name in ordered {
            // Idempotent-until-green: skip tools whose init already
            // succeeded in a previous run (same policy as init).
            if self.ddl_dir.manifest.is_inited(tool_name) {
                continue;
            }
            if let Some(t) = crate::platform::find_tool(tool_name) {
                match crate::installer::run_tool_init(t, self.verbose, self.non_interactive) {
                    Ok(()) => {
                        let _ = self.ddl_dir.record_inited(tool_name);
                    }
                    Err(e) => failures.push(StepFailure {
                        step: WizardStep::Delegate,
                        subject: tool_name.to_string(),
                        message: e.to_string(),
                    }),
                }
            }
        }
        failures
    }

    fn verify(&self) -> Vec<StepFailure> {
        // The same status report `ddl status` builds; any non-healthy
        // item is a verify failure for the resume summary.
        let report = crate::diagnostics::build_status_report(Some(&self.ddl_dir));
        let mut failures = Vec::new();
        for section in &report.sections {
            for item in &section.items {
                if item.level != genesis::status::StatusLevel::Healthy {
                    failures.push(StepFailure {
                        step: WizardStep::Verify,
                        subject: item.label.clone(),
                        message: item.value.clone(),
                    });
                }
            }
        }
        failures
    }
}
