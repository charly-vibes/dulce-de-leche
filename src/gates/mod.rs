//! `ddl init --gates` wiring — the end state users actually ask for.
//!
//! Responsibilities:
//! - lefthook.yml managed blocks: pre-commit (ah check, pretender gate,
//!   spk lint) and pre-push (ah check), inserted *inside* `commands:`,
//!   idempotent, and self-repairing against the espectacular mangle class
//!   (markers glued to content, block outside `commands:`, duplicates)
//! - .gitignore entries for tool data dirs (.testaruda/, .pretender/)
//! - beads no-db stamping in .beads/config.yaml
//! - init ordering: openspec init BEFORE ah init (ah needs openspec/)
//!
//! Read-only counterpart (DDL-6zn.5): `detect_lefthook_gates_problems`
//! inspects the on-disk wiring without mutating it — the repo-aware doctor
//! uses it as the single source of truth for "gates wired".

mod beads;
mod detect;
mod lefthook;
#[cfg(test)]
mod tests;
mod wiring;

pub use beads::*;
pub use detect::*;
pub use lefthook::*;
pub use wiring::*;
