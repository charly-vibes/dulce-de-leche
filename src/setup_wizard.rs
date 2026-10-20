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
//! The module is deliberately self-contained (std + serde only) so
//! tests/setup_wizard.rs can include it by path until §2.3 wires it into
//! the library.

use serde::Serialize;

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

        let resume_summary = steps
            .iter()
            .flat_map(|s| &s.failures)
            .map(|f| format!("{}: {} — {}", f.step.label(), f.subject, f.message))
            .collect();

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
