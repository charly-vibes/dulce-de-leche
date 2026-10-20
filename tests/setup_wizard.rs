//! Integration tests for the setup wizard step machine (DDL-g6w.2,
//! openspec add-setup-wizard-onboarding §2.2).
//!
//! Pins the five step transitions — pre-flight → migrate? → scope →
//! delegate → verify — against a scripted fake delegate. The step machine
//! orchestrates only; every real action flows through the
//! [`setup_wizard::WizardDelegate`] seams, backed (per §2.3) by the exact
//! installer/gates code paths `ddl init` uses.
//!
//! §2.3 wired the module into the library, so these tests import it via a
//! normal `dulce_de_leche::` path instead of a `#[path]` include.

use dulce_de_leche::setup_wizard::{
    MigrateDecision, PreFlightCheck, ScopeSelection, SetupWizard, StepFailure, WizardConfig,
    WizardDelegate, WizardStep,
};
use std::cell::RefCell;

/// Scripted delegate: records what the step machine asked for and replays
/// canned answers, so each test can pin one transition in isolation.
struct FakeDelegate {
    pre_flight_checks: Vec<PreFlightCheck>,
    migrate: MigrateDecision,
    migrate_non_interactive_seen: RefCell<Option<bool>>,
    cascade_scope_seen: RefCell<Option<ScopeSelection>>,
    cascade_failures: Vec<StepFailure>,
    verify_failures: Vec<StepFailure>,
}

impl FakeDelegate {
    fn clean() -> Self {
        Self {
            pre_flight_checks: vec![PreFlightCheck {
                name: "curl".into(),
                ok: true,
                fix: String::new(),
            }],
            migrate: MigrateDecision::Fresh,
            migrate_non_interactive_seen: RefCell::new(None),
            cascade_scope_seen: RefCell::new(None),
            cascade_failures: vec![],
            verify_failures: vec![],
        }
    }
}

impl WizardDelegate for FakeDelegate {
    fn pre_flight(&self) -> Vec<PreFlightCheck> {
        self.pre_flight_checks.clone()
    }

    fn migrate_decision(&self, non_interactive: bool) -> MigrateDecision {
        *self.migrate_non_interactive_seen.borrow_mut() = Some(non_interactive);
        self.migrate.clone()
    }

    fn scope_selection(&self) -> ScopeSelection {
        ScopeSelection::All
    }

    fn run_cascade(&mut self, scope: &ScopeSelection) -> Vec<StepFailure> {
        *self.cascade_scope_seen.borrow_mut() = Some(scope.clone());
        self.cascade_failures.clone()
    }

    fn verify(&self) -> Vec<StepFailure> {
        self.verify_failures.clone()
    }
}

fn config(tools: Vec<&str>, no_install: bool, yes: bool) -> WizardConfig {
    WizardConfig {
        tools: tools.into_iter().map(String::from).collect(),
        no_install,
        yes,
    }
}

// ---------------------------------------------------------------------------
// Transition 1+5: full run visits all five steps in order and reports ok.
// ---------------------------------------------------------------------------

#[test]
fn all_five_steps_run_in_order_and_report_ok() {
    let mut delegate = FakeDelegate::clean();
    let report = SetupWizard::new(config(vec![], false, true), &mut delegate).run();

    assert_eq!(
        WizardStep::ALL,
        [
            WizardStep::PreFlight,
            WizardStep::Migrate,
            WizardStep::Scope,
            WizardStep::Delegate,
            WizardStep::Verify,
        ],
        "canonical step order is pre-flight → migrate → scope → delegate → verify"
    );
    let visited: Vec<WizardStep> = report.steps.iter().map(|s| s.step).collect();
    assert_eq!(
        visited,
        WizardStep::ALL.to_vec(),
        "a clean run must execute every step exactly once, in order"
    );
    assert!(report.ok, "clean run must report ok, got {report:?}");
    assert!(
        report.resume_summary.is_empty(),
        "clean run must not produce a resume summary"
    );
}

// ---------------------------------------------------------------------------
// Transition 2: pre-flight failures surface the human-readable fix and fail
// the run (resume summary parity with init).
// ---------------------------------------------------------------------------

#[test]
fn pre_flight_failure_surfaces_fix_and_fails_run() {
    let mut delegate = FakeDelegate::clean();
    delegate.pre_flight_checks = vec![
        PreFlightCheck {
            name: "curl".into(),
            ok: true,
            fix: String::new(),
        },
        PreFlightCheck {
            name: "rustc".into(),
            ok: false,
            fix: "install rustup: https://rustup.rs".into(),
        },
    ];
    let report = SetupWizard::new(config(vec![], false, true), &mut delegate).run();

    let pre = &report.steps[0];
    assert_eq!(pre.step, WizardStep::PreFlight);
    assert!(!report.ok, "failing pre-flight must fail the run");
    let summary = report.resume_summary.join("\n");
    assert!(
        summary.contains("rustc") && summary.contains("https://rustup.rs"),
        "resume summary must name the failed check and its fix, got: {summary}"
    );
}

// ---------------------------------------------------------------------------
// Transition 3: the migrate? decision is delegated (never decided inline),
// and `-y` forwards non-interactive so the delegate skips its prompt.
// ---------------------------------------------------------------------------

#[test]
fn migrate_decision_is_delegated_and_yes_forwards_non_interactive() {
    let mut delegate = FakeDelegate::clean();
    delegate.migrate = MigrateDecision::Migrate {
        dry_run: vec!["would move ~/.config/foo → .ddl/foo".into()],
    };
    let report = SetupWizard::new(config(vec![], false, true), &mut delegate).run();

    assert_eq!(
        delegate.migrate_non_interactive_seen.into_inner(),
        Some(true),
        "--yes must forward non-interactive so the delegate skips its prompt"
    );
    let migrate = &report.steps[1];
    assert_eq!(migrate.step, WizardStep::Migrate);
    let detail = migrate.detail.to_string();
    assert!(
        detail.contains("dry_run") || detail.contains("Migrate"),
        "migrate outcome must record the delegated decision, got {detail}"
    );

    // Without -y the delegate is asked interactively.
    let mut delegate2 = FakeDelegate::clean();
    SetupWizard::new(config(vec![], false, false), &mut delegate2).run();
    assert_eq!(
        delegate2.migrate_non_interactive_seen.into_inner(),
        Some(false),
        "interactive runs must let the delegate prompt"
    );
}

// ---------------------------------------------------------------------------
// Transition 4: scope selection reaches the delegate cascade; explicit
// --tools narrows it; the delegate (not the wizard) owns the default.
// ---------------------------------------------------------------------------

#[test]
fn scope_flows_into_delegate_cascade_and_tools_narrow_it() {
    // Default: delegate's own selection (All) reaches the cascade untouched.
    let mut delegate = FakeDelegate::clean();
    SetupWizard::new(config(vec![], false, true), &mut delegate).run();
    assert_eq!(
        delegate.cascade_scope_seen.into_inner(),
        Some(ScopeSelection::All),
        "wizard must forward the delegated scope, not invent one"
    );

    // Explicit --tools narrows the scope before delegation.
    let mut delegate = FakeDelegate::clean();
    SetupWizard::new(config(vec!["ah", "spk"], false, true), &mut delegate).run();
    assert_eq!(
        delegate.cascade_scope_seen.into_inner(),
        Some(ScopeSelection::Categories(vec!["ah".into(), "spk".into()])),
        "--tools must narrow the delegated scope to the requested tools"
    );
}

// ---------------------------------------------------------------------------
// Transition 5: verify always runs — even after a cascade failure — and
// every failure lands in the resume summary (exit-non-zero contract).
// ---------------------------------------------------------------------------

#[test]
fn cascade_failure_does_not_skip_verify_and_fails_run() {
    let mut delegate = FakeDelegate::clean();
    delegate.cascade_failures = vec![StepFailure {
        step: WizardStep::Delegate,
        subject: "funkalization".into(),
        message: "no platform binary and no cargo fallback".into(),
    }];
    delegate.verify_failures = vec![StepFailure {
        step: WizardStep::Verify,
        subject: "ddl status".into(),
        message: "1 tool unhealthy".into(),
    }];
    let report = SetupWizard::new(config(vec![], false, true), &mut delegate).run();

    assert!(!report.ok);
    let visited: Vec<WizardStep> = report.steps.iter().map(|s| s.step).collect();
    assert_eq!(
        visited,
        WizardStep::ALL.to_vec(),
        "verify must run even when the cascade failed"
    );
    let summary = report.resume_summary.join("\n");
    assert!(
        summary.contains("funkalization") && summary.contains("ddl status"),
        "resume summary must cover cascade AND verify failures, got: {summary}"
    );
}
