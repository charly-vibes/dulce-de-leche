//! Health diagnostics — subprocess communication with each managed tool.
//!
//! ddl communicates with each tool via subprocess calls (`wai status --json`,
//! `dont status --json`). This decouples release cycles between ddl and
//! the tools it manages.

use std::process::Command;

use crate::error::Result;
use crate::platform::Tool;
use crate::dot_ddl::DdlDir;

/// Health status of a single managed tool.
#[derive(Debug, Clone)]
pub struct ToolHealth {
    pub name: &'static str,
    pub description: &'static str,
    pub installed: bool,
    pub version: Option<String>,
    pub config_ok: bool,
    pub status_check: ResultCheck,
    pub suggestion: Option<&'static str>,
}

/// Result of running a tool's status subcommand.
#[derive(Debug, Clone)]
pub enum ResultCheck {
    /// Command ran successfully (pass).
    Pass(String),
    /// Command ran with warnings (advisory).
    Warn(String),
    /// Command failed or not found.
    Fail(String),
    /// No status command defined for this tool.
    Skipped,
}

/// Run a tool's `--version` command and parse the output.
pub fn get_tool_version(tool: &Tool) -> Option<String> {
    let output = Command::new(tool.name).arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let first_line = stdout.lines().next()?;
    // Handle various output formats:
    //   "wai 2026.5.1"     -> "2026.5.1"
    //   "fabbro version dev-9875f7d" -> "dev-9875f7d"
    //   "ddl 0.1.0"         -> "0.1.0"
    let parts: Vec<&str> = first_line.split_whitespace().collect();
    if parts.len() >= 2 {
        // Skip the tool name, take the version
        let version = if parts[0].to_lowercase() == tool.name {
            parts[1]
        } else {
            parts[0]
        };
        // Skip keywords like "version"
        if version.to_lowercase() == "version" && parts.len() >= 3 {
            Some(parts[2].to_string())
        } else if version.starts_with(|c: char| c.is_ascii_digit() || c == 'v') {
            Some(version.trim_start_matches('v').to_string())
        } else {
            Some(version.to_string())
        }
    } else {
        Some(first_line.to_string())
    }
}

/// Run a tool's status command. Uses --json if available, falls back to --version.
pub fn run_tool_status(tool: &Tool) -> ResultCheck {
    // First try `status --json`
    let status_output = Command::new(tool.name)
        .args(["status", "--json"])
        .output();

    if let Ok(output) = status_output {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            return ResultCheck::Pass(stdout.trim().to_string());
        }
    }

    // Fallback: try `--version`
    let version_output = Command::new(tool.name)
        .arg("--version")
        .output();

    if let Ok(output) = version_output {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            return ResultCheck::Pass(stdout.trim().to_string());
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        return ResultCheck::Fail(format!(
            "{} --version failed (exit {}): {}",
            tool.name,
            output.status.code().unwrap_or(-1),
            stderr.trim()
        ));
    }

    ResultCheck::Fail(format!("{} not found on PATH", tool.name))
}

/// Run a tool's doctor/diagnostic command.
pub fn run_tool_doctor(tool: &Tool) -> ResultCheck {
    // Try `doctor` first, then `diagnostic`, fall back to status
    for cmd in &["doctor", "diagnostic", "check"] {
        let output = Command::new(tool.name).args([cmd, "--json"]).output();
        if let Ok(out) = output {
            if out.status.success() {
                let stdout = String::from_utf8_lossy(&out.stdout);
                return ResultCheck::Pass(stdout.trim().to_string());
            }
        }
    }
    // Fall back to status
    run_tool_status(tool)
}

/// Check if a tool's config directory exists under .ddl/ or at legacy locations.
pub fn check_tool_config(tool: &Tool, ddl_dir: &DdlDir) -> bool {
    // First check if tracked in manifest
    if ddl_dir.manifest.is_installed(tool.name) {
        return true;
    }

    // Check legacy config locations
    let legacy_paths: &[&str] = match tool.name {
        "wai" => &[".wai/config.toml"],
        "dont" => &[".dont/config.toml"],
        "ah" => &[".espectacular/config.toml"],
        "pretender" => &[".pretender.toml"],
        "testaruda" => &[".testaruda/config.toml"],
        "fabbro" => &[".fabbro/config.toml"],
        _ => &[],
    };

    if legacy_paths.iter().any(|p| std::path::Path::new(p).exists()) {
        return true;
    }

    // Check .ddl/<tool>/ directory
    let tool_path = ddl_dir.tool_path(tool.name);
    tool_path.exists()
}

/// Collect health info for all managed tools.
pub fn collect_all_health(ddl_dir: Option<&DdlDir>) -> Vec<ToolHealth> {
    use crate::platform::MANAGED_TOOLS;

    MANAGED_TOOLS
        .iter()
        .map(|tool| collect_tool_health(tool, ddl_dir))
        .collect()
}

/// Collect health info for a single tool.
pub fn collect_tool_health(tool: &Tool, ddl_dir: Option<&DdlDir>) -> ToolHealth {
    let installed = which(tool.name).is_some();
    let version = if installed { get_tool_version(tool) } else { None };

    // Config is OK if tracked in manifest OR legacy config exists
    let config_ok = ddl_dir
        .map(|d| {
            d.manifest.is_installed(tool.name) || check_tool_config(tool, d)
        })
        .unwrap_or(false);

    let status_check = if installed {
        run_tool_status(tool)
    } else {
        ResultCheck::Skipped
    };

    let suggestion = match (installed, config_ok) {
        (false, _) => Some("Run `ddl install {tool}`"),
        (true, false) => Some("Run `ddl init` to create config"),
        (true, true) => None,
    };

    ToolHealth {
        name: tool.name,
        description: tool.description,
        installed,
        version,
        config_ok,
        status_check,
        suggestion,
    }
}

/// Format a tool's health for terminal display.
pub fn format_health_line(health: &ToolHealth) -> String {
    let icon = if !health.installed {
        "○"
    } else if health.config_ok {
        "✓"
    } else {
        "⚠"
    };

    let version_str = health
        .version
        .as_ref()
        .map(|v| format!("v{}", v))
        .unwrap_or_else(|| "?".to_string());

    // Build a concise status summary
    let status_str = match &health.status_check {
        ResultCheck::Pass(s) => {
            let first_line = s.lines().next().unwrap_or(s).trim();
            // If it's JSON, just show a brief indicator
            if first_line.starts_with('{') || first_line.starts_with('[') {
                String::new()
            } else if first_line.len() > 40 {
                format!("  {}", &first_line[..37])
            } else {
                format!("  {}", first_line)
            }
        }
        ResultCheck::Warn(s) => format!("  ⚠ {}", s),
        ResultCheck::Fail(s) => format!("  ✗ {}", s),
        ResultCheck::Skipped => String::new(),
    };

    let config_str = if health.installed && !health.config_ok {
        "  no manifest".to_string()
    } else {
        String::new()
    };

    format!(
        "  {} {:12}  {}{}{}",
        icon, health.name, version_str, config_str, status_str
    )
}

/// Build a human-readable status summary.
pub fn status_summary(health: Vec<&ToolHealth>) -> String {
    let total = health.len();
    let ok = health.iter().filter(|h| h.installed && h.config_ok).count();
    let partial = health.iter().filter(|h| h.installed && !h.config_ok).count();
    let missing = health.iter().filter(|h| !h.installed).count();

    if missing == 0 && partial == 0 {
        format!("All {} tools are healthy. ✓", total)
    } else if missing == total {
        "No tools installed. Run `ddl init` to get started.".to_string()
    } else if ok == total {
        format!("All {} tools configured. ✓", total)
    } else {
        let mut parts = Vec::new();
        if ok > 0 {
            parts.push(format!("{} healthy", ok));
        }
        if partial > 0 {
            parts.push(format!("{} need config (`ddl init`)", partial));
        }
        if missing > 0 {
            parts.push(format!("{} not installed (`ddl install`)", missing));
        }
        format!("{} — {}", total, parts.join(", "))
    }
}

/// Check if a command exists on PATH.
pub fn which(cmd: &str) -> Option<std::path::PathBuf> {
    std::env::var_os("PATH").and_then(|paths| {
        for dir in std::env::split_paths(&paths) {
            let full = dir.join(cmd);
            if full.is_file() {
                return Some(full);
            }
            #[cfg(windows)]
            {
                let full_exe = dir.join(format!("{cmd}.exe"));
                if full_exe.is_file() {
                    return Some(full_exe);
                }
            }
        }
        None
    })
}

/// Run a comprehensive diagnostic on all tools and the .ddl/ environment.
pub fn run_full_diagnostic(ddl_dir: Option<&DdlDir>, fix: bool) -> Result<Vec<String>> {
    let mut messages = Vec::new();

    // Platform check
    match crate::platform::Platform::detect() {
        Some(p) => messages.push(format!("✓ Platform: {} ({})", p.os, p.arch)),
        None => messages.push("✗ Could not detect platform".to_string()),
    }

    // PATH check
    match std::env::var_os("PATH") {
        Some(path) => {
            let count = std::env::split_paths(&path).count();
            messages.push(format!("✓ PATH: {} entries", count));
        }
        None => messages.push("✗ PATH not set".to_string()),
    }

    // Prerequisites check
    for prereq in &["curl", "git", "cargo", "brew", "scoop"] {
        if which(prereq).is_some() {
            messages.push(format!("✓ {} found on PATH", prereq));
        }
    }

    // .ddl/ directory check
    messages.push(String::new());
    messages.push("── .ddl/ directory ──".to_string());

    match ddl_dir {
        Some(d) => {
            messages.push(format!("✓ .ddl/ at {}", d.path.display()));

            // Check manifest
            if d.manifest_path().exists() {
                let tool_count = d.manifest.tools.len();
                let installed_count = d
                    .manifest
                    .tools
                    .values()
                    .filter(|e| e.status == "installed")
                    .count();
                messages.push(format!(
                    "✓ manifest.json: {} tools tracked, {} installed",
                    tool_count, installed_count
                ));
            } else {
                messages.push("✗ manifest.json not found".to_string());
                if fix {
                    d.save_manifest()?;
                    messages.push("  ✓ manifest.json created".to_string());
                }
            }

            // Check config
            if d.config_path().exists() {
                messages.push("✓ config.toml found".to_string());
            } else {
                messages.push("○ config.toml not found (created on next init)".to_string());
            }

            // Broken symlinks
            let broken = d.detect_broken_symlinks();
            if broken.is_empty() {
                messages.push("✓ no broken symlinks".to_string());
            } else {
                for symlink in &broken {
                    messages.push(format!("✗ broken symlink: {}", symlink.display()));
                    if fix {
                        let _ = std::fs::remove_file(symlink)
                            .or_else(|_| std::fs::remove_dir(symlink));
                        messages.push(format!("  ✓ removed broken symlink: {}", symlink.display()));
                    }
                }
            }
        }
        None => {
            messages.push("○ No .ddl/ directory found".to_string());
            if fix {
                let created = DdlDir::create_at(&std::path::PathBuf::from(".ddl"))?;
                messages.push("  ✓ .ddl/ directory created".to_string());
                // Re-run diagnostics on the newly created directory
                return run_full_diagnostic(Some(&created), false);
            }
            messages.push("  Run `ddl init` to create one.".to_string());
        }
    }

    // Per-tool diagnostics
    messages.push(String::new());
    messages.push("── Tool diagnostics ──".to_string());

    for tool in crate::platform::MANAGED_TOOLS {
        let installed = which(tool.name).is_some();
        let version = installed.then(|| get_tool_version(tool)).flatten();

        let line = match (installed, version) {
            (true, Some(v)) => {
                // Run doctor subcommand
                let doctor_result = run_tool_doctor(tool);
                match doctor_result {
                    ResultCheck::Pass(s) => {
                        let summary = s.lines().next().unwrap_or(&s);
                        format!("✓ {:12} v{} — {}", tool.name, v, summary)
                    }
                    ResultCheck::Warn(s) => format!("⚠ {:12} v{} — {}", tool.name, v, s),
                    ResultCheck::Fail(s) => format!("✗ {:12} v{} — {}", tool.name, v, s),
                    ResultCheck::Skipped => format!("✓ {:12} v{}", tool.name, v),
                }
            }
            (true, None) => {
                format!("⚠ {:12} found on PATH but version unknown", tool.name)
            }
            (false, _) => format!("○ {:12} not installed", tool.name),
        };
        messages.push(line);

        // Auto-fix: suggest install
        if fix && !installed {
            messages.push(format!("  ℹ  Run `ddl install {}` to install", tool.name));
        }
    }

    Ok(messages)
}