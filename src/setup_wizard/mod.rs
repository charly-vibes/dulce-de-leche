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

mod delegate;
mod steps;
mod wizard;

pub use delegate::*;
pub use steps::*;
pub use wizard::*;
