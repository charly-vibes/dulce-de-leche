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
// §2.3 reuse-contract guard: the wizard must never fork the init
// resume-summary format. Delegate-step failures are the wizard's analog of
// init-cascade failures, so they must render through init's OWN summary
// builder (`installer::resume_summary`) — byte-for-byte. If the wizard
// forks the format (its own format! string), this test fails on the next
// init-side change to the summary contract.
// ---------------------------------------------------------------------------

#[test]
fn guard_delegate_failures_render_through_init_summary_builder() {
    use dulce_de_leche::installer::{InitFailure, resume_summary};

    let mut delegate = FakeDelegate::clean();
    delegate.cascade_failures = vec![
        StepFailure {
            step: WizardStep::Delegate,
            subject: "funkalization".into(),
            message: "no platform binary and no cargo fallback".into(),
        },
        StepFailure {
            step: WizardStep::Delegate,
            subject: "openspec".into(),
            message: "refused re-init".into(),
        },
    ];
    let report = SetupWizard::new(config(vec![], false, true), &mut delegate).run();

    let expected = resume_summary(&[
        InitFailure {
            tool: "funkalization".into(),
            detail: "no platform binary and no cargo fallback".into(),
        },
        InitFailure {
            tool: "openspec".into(),
            detail: "refused re-init".into(),
        },
    ]);
    assert_eq!(
        report.resume_summary, expected,
        "wizard delegate-step failures must render through installer::resume_summary, \
         not a forked format"
    );
}

// ---------------------------------------------------------------------------
// §2.3 guard (production delegate): InitPathsDelegate must DERIVE its
// pre-flight checks from the installer's own method selection and
// prerequisite checks — never a forked hardcoded list. If either side
// drifts (installer adds/renames a method, or the wizard hardcodes its
// own list), this shared-path assertion fails.
// ---------------------------------------------------------------------------

#[test]
fn guard_production_delegate_preflight_derives_from_installer_methods() {
    use dulce_de_leche::dot_ddl::DdlDir;
    use dulce_de_leche::installer::{InstallMethod, best_install_method};
    use dulce_de_leche::platform::{MANAGED_TOOLS, Platform};
    use dulce_de_leche::setup_wizard::InitPathsDelegate;

    let temp = tempfile::tempdir().unwrap();
    let ddl_dir = DdlDir::create_at(temp.path()).unwrap();
    let platform = Platform::detect().unwrap();
    // Expectation derived from the SAME scan the migrate path performs.
    let legacy_count = ddl_dir.detect_legacy_configs().len();
    let delegate = InitPathsDelegate::new(ddl_dir, platform.clone(), false, false, true);

    // Independent recomputation of what init's install paths require.
    let mut seen: Vec<InstallMethod> = Vec::new();
    let expected: Vec<(String, bool)> = MANAGED_TOOLS
        .iter()
        .filter_map(|tool| {
            let method = best_install_method(tool, &platform);
            if seen.contains(&method) {
                None
            } else {
                seen.push(method.clone());
                Some((method.to_string(), method.check_prerequisites().is_ok()))
            }
        })
        .collect();

    let checks = delegate.pre_flight();
    let actual: Vec<(String, bool)> = checks.iter().map(|c| (c.name.clone(), c.ok)).collect();
    assert_eq!(
        actual, expected,
        "wizard pre-flight must check exactly the install methods \
         init's install paths select, via InstallMethod::check_prerequisites"
    );

    // The migrate seam must route through the same legacy-config scan
    // ddl migrate uses: the decision mirrors detect_legacy_configs()
    // exactly (one dry-run line per legacy config), never a forked heuristic.
    match delegate.migrate_decision(true) {
        MigrateDecision::Fresh => assert_eq!(
            legacy_count, 0,
            "Fresh decision but the migrate scan found legacy configs"
        ),
        MigrateDecision::Migrate { dry_run } => {
            assert!(legacy_count > 0, "Migrate decision but the scan found none");
            assert_eq!(
                dry_run.len(),
                legacy_count,
                "dry-run report must cover every legacy config the migrate scan found"
            );
        }
    }

    // The verify seam must route through the ddl status report: every
    // verify failure names a managed tool (not an invented subject).
    for failure in delegate.verify() {
        assert!(
            MANAGED_TOOLS.iter().any(|t| t.name == failure.subject),
            "verify failures must come from the ddl status report, got: {:?}",
            failure
        );
    }
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
