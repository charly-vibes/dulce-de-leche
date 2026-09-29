//! Error types for ddl.

use miette::Diagnostic;
use thiserror::Error;

/// Top-level error type for all ddl operations.
#[derive(Error, Debug, Diagnostic)]
pub enum DdlError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("TOML parse error: {0}")]
    Toml(#[from] toml::de::Error),

    #[error("Platform not supported: {0}")]
    UnsupportedPlatform(String),

    #[error("Tool not found: {0}")]
    #[diagnostic(help("Run `ddl status` to see available tools"))]
    ToolNotFound(String),

    #[error("Installation failed: {0}")]
    #[diagnostic(help("Check network connectivity and try again"))]
    InstallFailed(String),

    // Note: no #[diagnostic(help)] on this variant — miette's derive mis-lints
    // named-field variants (unused_assignments false positive); the remedy text
    // is carried by the InstallFailed message built at the fallback site.
    #[error("No release binary for {tool}: {url}")]
    NoReleaseBinary { tool: String, url: String },

    // Note: no #[diagnostic(help)] on this variant — same miette derive
    // mis-lint as NoReleaseBinary; the remedy is spelled out in the message.
    // Note also: this defends against corruption / mismatched re-publish. A
    // release whose checksums.txt itself lists the attacker's digest is not
    // caught — that would require release signing (e.g. sigstore).
    #[error(
        "Checksum verification failed for {tool}: downloaded asset does not match the release checksum (expected sha256 {expected}, got {actual}) — {url}. The download was NOT installed; it may be corrupted or tampered with. Retry, or verify the release manually."
    )]
    ChecksumMismatch {
        tool: String,
        expected: String,
        actual: String,
        url: String,
    },

    #[error("Prerequisite missing: {0}")]
    #[diagnostic(help("Install the prerequisite and try again"))]
    PrerequisiteMissing(String),

    #[error("Incompatible version: {0}")]
    #[diagnostic(help("Try upgrading ddl first"))]
    VersionMismatch(String),

    #[error("Partial failure — some operations completed, some failed")]
    PartialFailure,

    #[error("{0}")]
    Other(String),
}

/// Convenience alias for Result types.
pub type Result<T> = std::result::Result<T, DdlError>;
