//! Restricted-privilege child policy: env allowlist and execution scopes (DDL-uu8, SEC-04).

use super::*;
use crate::error::{DdlError, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

pub const ALLOW_GLOBAL_SKILL_ENV: &str = "DDL_ALLOW_GLOBAL_SKILL";

/// Legacy-transition variable: when set to `1`, children inherit the caller's
/// full environment instead of the restricted allowlist. Mirrors the
/// `DDL_ALLOW_UNVERIFIED_RELEASE` explicit-authorization semantics.
pub const ALLOW_UNRESTRICTED_CHILD_ENV: &str = "DDL_ALLOW_UNRESTRICTED_CHILD_ENV";

/// The execution scope of a child spawned by ddl.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionScope {
    /// Package build/install (`cargo install`, `npm install -g`) — authorized
    /// by the explicit user-invoked install/upgrade command itself.
    PackageBuild,
    /// Global skill installation — writes outside this project and affects
    /// other projects; requires explicit authorization via
    /// [`ALLOW_GLOBAL_SKILL_ENV`].
    GlobalSkill,
    /// Per-tool init cascade — writes are confined to the invoking project
    /// tree and ddl's own bin dir; authorized by the explicit `ddl init`.
    ToolInit,
}

impl ExecutionScope {
    pub fn label(self) -> &'static str {
        match self {
            Self::PackageBuild => "package-build",
            Self::GlobalSkill => "global-skill",
            Self::ToolInit => "tool-init",
        }
    }

    /// The env var granting explicit authorization for this scope, if any.
    /// Scopes writing only inside this project (or explicitly requested by
    /// the user's own install/upgrade command) need no additional grant.
    pub fn authorization_env(self) -> Option<&'static str> {
        match self {
            Self::GlobalSkill => Some(ALLOW_GLOBAL_SKILL_ENV),
            Self::PackageBuild | Self::ToolInit => None,
        }
    }
}

/// Platform support matrix: the restricted-privilege mechanism (controlled
/// env dictionary + explicit scope authorization) is portable across every
/// platform std supports. Any other target must be rejected explicitly —
/// never silently run children unsandboxed.
pub fn child_policy_supported() -> bool {
    cfg!(any(
        target_os = "linux",
        target_os = "macos",
        target_os = "windows"
    ))
}

/// Environment allowlist for children spawned by ddl. Everything NOT listed
/// here (credentials, tokens, secrets) is dropped from the child environment.
#[cfg(not(windows))]
pub const CHILD_ENV_ALLOWLIST: &[&str] = &[
    "PATH",
    "HOME",
    "USER",
    "LOGNAME",
    "SHELL",
    "TERM",
    "TMPDIR",
    "LANG",
    "LC_ALL",
    "XDG_CONFIG_HOME",
    "XDG_DATA_HOME",
    "XDG_CACHE_HOME",
    "XDG_RUNTIME_DIR",
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "NO_PROXY",
    "http_proxy",
    "https_proxy",
    "no_proxy",
    "SSL_CERT_FILE",
    "SSL_CERT_DIR",
    "CARGO_HOME",
    "RUSTUP_HOME",
    "NPM_CONFIG_PREFIX",
];

/// Environment allowlist for children spawned by ddl (Windows variants:
/// cmd shim routing and npm/cargo need the system profile variables).
#[cfg(windows)]
pub const CHILD_ENV_ALLOWLIST: &[&str] = &[
    "PATH",
    "PATHEXT",
    "COMSPEC",
    "SYSTEMROOT",
    "WINDIR",
    "SystemDrive",
    "USERPROFILE",
    "HOMEDRIVE",
    "HOMEPATH",
    "APPDATA",
    "LOCALAPPDATA",
    "PROGRAMDATA",
    "ALLUSERSPROFILE",
    "PROGRAMFILES",
    "COMMONPROGRAMFILES",
    "TEMP",
    "TMP",
    "USERNAME",
    "USERDOMAIN",
    "COMPUTERNAME",
    "LANG",
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "NO_PROXY",
    "SSL_CERT_FILE",
    "SSL_CERT_DIR",
    "CARGO_HOME",
    "RUSTUP_HOME",
    "NPM_CONFIG_PREFIX",
];

/// Is the legacy unrestricted-child-environment transition explicitly
/// authorized?
pub(crate) fn unrestricted_child_env_authorized() -> bool {
    std::env::var(ALLOW_UNRESTRICTED_CHILD_ENV).as_deref() == Ok("1")
}

/// Build the child environment from the policy allowlist, applying explicit
/// overrides last (so e.g. the tool-init PATH prepend always wins).
///
/// Fail-closed: without a usable PATH the child cannot be started sanely, so
/// the policy refuses instead of inheriting a half-broken environment.
pub fn sanitize_child_env(
    overrides: &[(&str, std::ffi::OsString)],
) -> Result<Vec<(String, std::ffi::OsString)>> {
    let mut env: Vec<(String, std::ffi::OsString)> = Vec::new();
    if unrestricted_child_env_authorized() {
        for (k, v) in std::env::vars_os() {
            env.push((k.to_string_lossy().into_owned(), v));
        }
    } else {
        for key in CHILD_ENV_ALLOWLIST {
            if let Some(v) = std::env::var_os(key) {
                env.push(((*key).to_string(), v));
            }
        }
    }
    for (k, v) in overrides {
        env.retain(|(existing, _)| existing.as_str() != *k);
        env.push(((*k).to_string(), v.clone()));
    }
    if !env.iter().any(|(k, _)| k == "PATH") {
        return Err(DdlError::InstallFailed(
            "child environment policy (DDL-uu8): PATH is missing — refusing to spawn a child without an executable search path".to_string(),
        ));
    }
    Ok(env)
}

/// Apply the restricted-privilege policy to a child command: replace the
/// inherited environment with the allowlisted one and reject execution on
/// unsupported platforms. Zero silent unsandboxed fallback.
pub fn apply_child_policy(
    command: &mut Command,
    scope: ExecutionScope,
    overrides: &[(&str, std::ffi::OsString)],
) -> Result<()> {
    if !child_policy_supported() {
        return Err(DdlError::UnsupportedPlatform(format!(
            "restricted child execution policy (DDL-uu8) has no enforcement mechanism on this platform — refusing to run the {} child unsandboxed",
            scope.label()
        )));
    }
    let env = sanitize_child_env(overrides)?;
    command.env_clear();
    for (k, v) in env {
        command.env(k, v);
    }
    Ok(())
}

/// Approve (or reject) a child execution scope. Scopes with an authorization
/// env var are rejected unless it is explicitly set to `1`; the rejection
/// message names the variable so the remedy is discoverable.
pub fn authorize_scope(scope: ExecutionScope) -> Result<()> {
    scope_authorize_with(scope, |var| std::env::var(var).ok())
}

/// Pure decision core of [`authorize_scope`], injectable for tests.
pub fn scope_authorize_with<F>(scope: ExecutionScope, lookup: F) -> Result<()>
where
    F: Fn(&str) -> Option<String>,
{
    match scope.authorization_env() {
        // Authorized by the explicit user-invoked install/upgrade/init command.
        None => Ok(()),
        Some(var) if lookup(var).as_deref() == Some("1") => Ok(()),
        Some(var) => Err(DdlError::InstallFailed(format!(
            "{} scope rejected by the restricted-privilege policy (DDL-uu8): its writes reach beyond this project and affect other projects. Re-run with {var}=1 to explicitly authorize, or install skills locally instead.",
            scope.label()
        ))),
    }
}

/// The approved write roots for a child execution scope. Anything outside
/// these roots is outside the child's declared filesystem authority.
pub fn child_write_roots(scope: ExecutionScope, project_root: Option<&Path>) -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();
    if let Some(project) = project_root {
        roots.push(project.to_path_buf());
    }
    if let Some(bin) = ddl_bin_dir() {
        roots.push(bin);
    }
    match scope {
        // Global package registries/caches (cargo home) are the package-build
        // scope's declared target.
        ExecutionScope::PackageBuild => {
            if let Some(home) = std::env::var_os("CARGO_HOME") {
                roots.push(PathBuf::from(home));
            }
        }
        // Global skills live under ~/.agents/skills — the global-skill
        // scope's declared target.
        ExecutionScope::GlobalSkill => {
            if let Some(dir) = crate::skills::global_skills_dir() {
                roots.push(dir);
            }
        }
        ExecutionScope::ToolInit => {}
    }
    roots
}

/// Reject a path outside the approved write roots of `scope`.
pub fn check_write_root(
    scope: ExecutionScope,
    path: &Path,
    project_root: Option<&Path>,
) -> Result<()> {
    let roots = child_write_roots(scope, project_root);
    if roots.iter().any(|root| path.starts_with(root)) {
        Ok(())
    } else {
        let listed = roots
            .iter()
            .map(|r| r.display().to_string())
            .collect::<Vec<_>>()
            .join(", ");
        Err(DdlError::InstallFailed(format!(
            "write to {} is outside the approved roots of the {} scope (DDL-uu8): approved roots: {listed}",
            path.display(),
            scope.label()
        )))
    }
}

pub(crate) fn which(cmd: &str) -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|paths| {
        for dir in std::env::split_paths(&paths) {
            let full = dir.join(cmd);
            if full.is_file() {
                return Some(full);
            }
            #[cfg(windows)]
            {
                let full_exe = dir.join(std::path::PathBuf::from(cmd).with_extension("exe"));
                if full_exe.is_file() {
                    return Some(full_exe);
                }
                // npm-ecosystem tools are .cmd shims on Windows — probe them
                // so check_prerequisites doesn't fail on a working npm.
                for ext in ["cmd", "bat"] {
                    let shim = dir.join(format!("{cmd}.{ext}"));
                    if shim.is_file() {
                        return Some(shim);
                    }
                }
            }
        }
        None
    })
}
