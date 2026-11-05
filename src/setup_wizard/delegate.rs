//! Production delegate backing the wizard seams with the exact ddl init/install code paths.

use super::*;
use crate::dot_ddl::DdlDir;
use crate::installer::InstallMethod;
use crate::platform::Platform;

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
