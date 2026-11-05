//! Installation chain — binary download (primary, sha256-verified), cargo
//! install (fallback), npm install.
//!
//! Policy (DDL-ei3): prebuilt binary first on every platform — no prerequisites
//! beyond curl/wget. Binary downloads are verified against the release's
//! published checksums (DDL-5ph) before extraction; mismatch aborts the
//! install. When the release has no binary for the platform (404),
//! fall back to `cargo install` if cargo is available; otherwise fail with an
//! error naming both remedies. npm-distributed tools (incitaciones) always
//! install via npm. Homebrew and Scoop are not part of ddl's install decisions;
//! their formulas/manifests remain published for manual installs.
//!
//! Rationale (DDL-yts, SEC-03): verification must be *available*, not merely
//! attempted. A release that publishes no usable checksum — missing,
//! malformed, incomplete, or transport-failed metadata — cannot be
//! authenticated, so the download is REJECTED before any destination
//! mutation, instead of being installed with a verbose-only skip warning.
//! Provenance verification (DDL-wof, SEC-03/06) binds a release to its
//! publisher/build identity: the release workflow publishes a
//! `provenance.json` manifest naming the repository, workflow and tag that
//! built each asset, and the installer rejects artifacts whose manifest names
//! an unexpected repository/workflow/tag or whose bytes diverge from the
//! manifest's per-asset digest. The trust roots are the constants
//! `EXPECTED_PROVENANCE_WORKFLOW`; the legacy transition
//! (`DDL_ALLOW_UNVERIFIED_RELEASE=1`) may authorize a *missing* manifest but
//! never a *contradicting* one.
//!
//! Rationale (DDL-uu8, SEC-04): children spawned by ddl (cargo, npm, npx,
//! tool-init commands) must not silently inherit the caller's full environment
//! and filesystem authority. Every such child runs with an explicit
//! restricted-privilege policy: an allowlisted environment (credentials and
//! other non-approved variables are dropped), and execution scopes whose
//! write roots reach beyond this project (global skill installs) are gated
//! behind explicit authorization env vars. Enforcement mechanism is
//! deliberately cheap and portable — controlled env dictionary + explicit
//! scope authorization, fail-closed on unsupported platforms — with NO silent
//! unsandboxed fallback; kernel-level sandboxing would require a verifier
//! dependency, which the ticket explicitly reserves for a separate design
//! approval.
//!
//! Rationale (DDL-4oi, SEC-01): archive extraction must never write directly
//! to the installed destination. The expected binary is extracted as the only
//! accepted entry — regular files only (links and special files rejected),
//! size-bounded, traversal-safe — into an *exclusive* staging directory on
//! the destination's filesystem, then atomically swapped into place.
//!
//! Rationale (DDL-5bs, SEC-04): replay resolves the artifact purely from the
//! recorded lock identity; only explicit upgrades resolve latest.
//!
//! Rationale (DDL-1z1, SEC-02): bootstrap consumes finite host resources —
//! release bodies, extracted files, captured child output and wall-clock
//! time — each governed by an explicit numeric budget enforced fail-closed.

mod child;
mod download;
mod exec;
mod init;
mod install;
mod method;
mod probe;
mod provenance;
mod staging;
mod upgrade;

pub use child::*;
pub(crate) use download::*;
pub use exec::*;
pub use init::*;
pub use install::*;
pub use method::*;
pub use probe::*;
pub use provenance::*;
pub use staging::*;
pub use upgrade::*;

/// Print a trace message to stderr when `verbose` is enabled.
pub(crate) fn verbose_print(verbose: bool, msg: &str) {
    if verbose {
        eprintln!("  · {msg}");
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod tests_verify;
