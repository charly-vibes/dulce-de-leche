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
