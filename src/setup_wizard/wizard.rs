//! SetupWizard orchestration: drive the five steps, fold failures, emit resume summaries.

use super::*;

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
