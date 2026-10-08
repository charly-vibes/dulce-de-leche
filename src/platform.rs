//! Platform detection — OS, architecture, and available package managers.

use serde::{Deserialize, Serialize};
use std::fmt;

/// A detected platform triple.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Platform {
    pub os: Os,
    pub arch: Arch,
}

impl Platform {
    /// Detect the current platform at runtime.
    pub fn detect() -> Option<Self> {
        let os = Os::detect()?;
        let arch = Arch::detect()?;
        Some(Self { os, arch })
    }

    /// Human-readable platform string (e.g., "macos-arm64").
    pub fn as_str(&self) -> String {
        format!("{}-{}", self.os.as_str(), self.arch.as_str())
    }
}

/// Operating system.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Os {
    Macos,
    Linux,
    Windows,
}

impl Os {
    pub fn detect() -> Option<Self> {
        match std::env::consts::OS {
            "macos" => Some(Self::Macos),
            "linux" => Some(Self::Linux),
            "windows" => Some(Self::Windows),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Macos => "darwin",
            Self::Linux => "linux",
            Self::Windows => "windows",
        }
    }
}

impl fmt::Display for Os {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// CPU architecture.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Arch {
    Amd64,
    Arm64,
}

impl Arch {
    pub fn detect() -> Option<Self> {
        match std::env::consts::ARCH {
            "x86_64" => Some(Self::Amd64),
            "aarch64" => Some(Self::Arm64),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Amd64 => "amd64",
            Self::Arm64 => "arm64",
        }
    }
}

impl fmt::Display for Arch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Available package managers on the current system.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PackageManager {
    Brew,
    Cargo,
    Scoop,
    Binary,
}

impl PackageManager {
    /// Check if the given package manager is available on the current system.
    pub fn is_available(&self) -> bool {
        match self {
            Self::Brew => which("brew").is_some(),
            Self::Cargo => which("cargo").is_some(),
            Self::Scoop => which("scoop").is_some(),
            Self::Binary => true, // always available (curl/wget assumed)
        }
    }
}

/// Check if a command exists on PATH.
fn which(cmd: &str) -> Option<std::path::PathBuf> {
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
            }
        }
        None
    })
}

/// Adoption tier of a managed tool (DDL-3i4, from the usage census).
///
/// Mechanics only — enforcement per tier is deliberately NOT implemented
/// here; that decision is parked behind the dogfood epic (charly-26y).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolCategory {
    /// Daily-use spine of the ecosystem.
    Core,
    /// Actively used companions worth installing by default.
    Recommended,
    /// Opt-in tools with little or no observed adoption.
    Extension,
}

impl ToolCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Core => "core",
            Self::Recommended => "recommended",
            Self::Extension => "extension",
        }
    }
}

impl std::fmt::Display for ToolCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Development maturity of a managed tool (§5 vocabulary, DDL-57e).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Maturity {
    /// Public API frozen; semver; breaking changes only via major.
    Stable,
    /// Core works, API may still shift; used in anger by ≥1 other ecosystem tool.
    Beta,
    /// Exploratory; may be renamed or sunset.
    Experimental,
    /// Maintenance mode; no new features; CI shrinks to build+test.
    Sunset,
}

impl Maturity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Stable => "stable",
            Self::Beta => "beta",
            Self::Experimental => "experimental",
            Self::Sunset => "sunset",
        }
    }
}

impl std::fmt::Display for Maturity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A managed tool in the charly-vibes ecosystem.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    pub name: &'static str,
    pub description: &'static str,
    pub crate_name: &'static str,
    pub formula_name: &'static str,
    pub repo: &'static str,
    /// npm package name when the tool is npm-distributed; `None` for
    /// brew/cargo/scoop/binary tools. Drives install method, version
    /// detection, and upgrade behavior.
    #[serde(default)]
    pub npm_package: Option<&'static str>,
    /// Adoption tier (core/recommended/extension) from the usage census.
    pub category: ToolCategory,
    /// Development maturity (stable/beta/experimental/sunset, §5).
    pub maturity: Maturity,
}

impl Tool {
    /// Release-asset prefix used by binary downloads. Defaults to the tool
    /// name; overridden when a project publishes assets under a different
    /// prefix (e.g. beads ships `beads_<ver>_<target>` archives containing
    /// the `bd` binary).
    pub fn binary_asset_prefix(&self) -> &'static str {
        match self.name {
            "bd" => "beads",
            _ => self.name,
        }
    }

    /// Whether this tool can be installed with `cargo install <crate_name>`.
    /// False when `crate_name` is not a crates.io package (Go tools, npm
    /// tools) — the cargo fallback must never run for these, or it would
    /// install an unrelated crate.
    pub fn cargo_installable(&self) -> bool {
        !self.crate_name.is_empty()
    }
}

/// All managed tools.
pub const MANAGED_TOOLS: &[Tool] = &[
    Tool {
        name: "wai",
        description: "Workflow manager for AI-driven development",
        crate_name: "wai-cli",
        formula_name: "wai",
        repo: "charly-vibes/wai",
        npm_package: None,
        category: ToolCategory::Core,
        maturity: Maturity::Stable,
    },
    Tool {
        name: "dont",
        description: "Epistemic discipline for AI-driven development",
        crate_name: "dont-cli",
        formula_name: "dont",
        repo: "charly-vibes/dont",
        npm_package: None,
        category: ToolCategory::Recommended,
        maturity: Maturity::Beta,
    },
    Tool {
        name: "ah",
        description: "Behavioral specification testing",
        crate_name: "espectacular",
        formula_name: "ah",
        repo: "charly-vibes/espectacular",
        npm_package: None,
        category: ToolCategory::Recommended,
        maturity: Maturity::Beta,
    },
    Tool {
        name: "pretender",
        description: "Code quality automation",
        crate_name: "pretender",
        formula_name: "pretender",
        repo: "charly-vibes/pretender",
        npm_package: None,
        category: ToolCategory::Extension,
        maturity: Maturity::Beta,
    },
    Tool {
        name: "testaruda",
        description: "Test selection and prioritization",
        crate_name: "testaruda",
        formula_name: "testaruda",
        repo: "charly-vibes/testaruda",
        npm_package: None,
        category: ToolCategory::Core,
        maturity: Maturity::Stable,
    },
    Tool {
        name: "vampiro",
        description: "Cross-language composition checking at call boundaries",
        crate_name: "vampiro",
        formula_name: "vampiro",
        repo: "charly-vibes/vampiro",
        npm_package: None,
        category: ToolCategory::Extension,
        maturity: Maturity::Beta,
    },
    Tool {
        name: "bd",
        description: "Issue tracker with first-class dependency support (beads)",
        crate_name: "",
        formula_name: "beads",
        repo: "gastownhall/beads",
        npm_package: None,
        category: ToolCategory::Extension,
        maturity: Maturity::Stable,
    },
    Tool {
        name: "openspec",
        description: "Spec-driven development workflow for AI coding agents",
        crate_name: "@fission-ai/openspec",
        formula_name: "openspec",
        repo: "fission-ai/openspec",
        npm_package: Some("@fission-ai/openspec"),
        category: ToolCategory::Extension,
        maturity: Maturity::Beta,
    },
    Tool {
        name: "incitaciones",
        description: "Reusable prompts and skills for CLI LLM tools",
        crate_name: "incitaciones",
        formula_name: "incitaciones",
        repo: "charly-vibes/incitaciones",
        npm_package: Some("incitaciones"),
        category: ToolCategory::Extension,
        maturity: Maturity::Experimental,
    },
    Tool {
        name: "turu",
        description: "Deterministic knowledge workspace management (whisper-vibes)",
        crate_name: "whisper-vibes",
        formula_name: "turu",
        repo: "charly-vibes/whisper",
        npm_package: None,
        category: ToolCategory::Recommended,
        maturity: Maturity::Beta,
    },
    Tool {
        name: "specodelic",
        description: "Markdown spec format (Intent/Constraints/Model/Properties) and the CLI that lints, compiles, verifies, and refactors it",
        crate_name: "specodelic",
        formula_name: "specodelic",
        repo: "charly-vibes/specodelic",
        npm_package: None,
        category: ToolCategory::Extension,
        maturity: Maturity::Experimental,
    },
];

/// Find a tool by name (case-insensitive; matches the canonical name or
/// the crate/binary alias — e.g. both `ah` and `espectacular` resolve to
/// the espectacular tool).
pub fn find_tool(name: &str) -> Option<&'static Tool> {
    MANAGED_TOOLS.iter().find(|t| {
        t.name.eq_ignore_ascii_case(name)
            || (!t.crate_name.is_empty() && t.crate_name.eq_ignore_ascii_case(name))
    })
}

/// Names that refer to ddl itself rather than a managed tool. Agents type
/// these after 404ing on `charly-vibes/ddl` — the alias must resolve to a
/// teachable error, not a bare "Unknown tool" (DDL-6zn.8).
pub const SELF_NAMES: &[&str] = &["ddl", "dulce", "dulce-de-leche"];

/// Is `name` a user-supplied spelling of ddl itself?
pub fn is_self_name(name: &str) -> bool {
    SELF_NAMES.iter().any(|n| n.eq_ignore_ascii_case(name))
}

/// The error message for a self-name lookup — the teachable moment that
/// names the canonical repository and the real update path.
pub fn self_name_message(name: &str) -> String {
    format!(
        "'{name}' is this tool (ddl), not a managed tool. ddl's canonical \
         repository is charly-vibes/dulce-de-leche (binary: ddl, crate: \
         dulce-de-leche) — looking up charly-vibes/ddl returns 404. Update \
         ddl itself with `cargo install dulce-de-leche` or the binary \
         download from the repository releases."
    )
}

/// Resolve a user-supplied tool name: case-insensitive, accepts the
/// canonical name or its crate/binary alias. Self-names (`ddl`, `dulce`,
/// `dulce-de-leche`) and unknown names produce a teachable error message
/// (single source of truth shared by `install`, `upgrade`, and `--tools`).
pub fn resolve_tool(name: &str) -> Result<&'static Tool, String> {
    if is_self_name(name) {
        return Err(self_name_message(name));
    }
    find_tool(name).ok_or_else(|| {
        let names: Vec<&str> = MANAGED_TOOLS.iter().map(|t| t.name).collect();
        match did_you_mean(name, &names) {
            Some(s) => format!("Unknown tool '{name}'. Did you mean '{s}'?"),
            None => format!("Unknown tool '{name}'"),
        }
    })
}

/// Simple "did you mean?" suggestion using genesis SuggestionEngine.
/// Returns the closest matching tool name if within threshold.
pub fn did_you_mean(input: &str, candidates: &[&str]) -> Option<String> {
    use genesis::suggestions::{CommandRegistry, SuggestionEngine};

    let mut registry = CommandRegistry::new();
    registry.register("ddl", candidates.iter().map(|c| c.to_string()).collect());

    let engine = SuggestionEngine::new();
    match engine.suggest_typo(input, &registry) {
        Some(genesis::suggestions::Suggestion::DidYouMean { suggestion, .. }) => Some(suggestion),
        _ => None,
    }
}

#[cfg(test)]
mod alias_tests {
    use super::*;

    /// DDL-6zn.8: binary-vs-crate confusion (ah⇄espectacular) caused a real
    /// CI exit-127 — both names must resolve to the same tool.
    #[test]
    fn find_tool_matches_crate_name() {
        assert_eq!(find_tool("espectacular").map(|t| t.name), Some("ah"));
        assert_eq!(find_tool("whisper-vibes").map(|t| t.name), Some("turu"));
    }

    /// The docstring always claimed case-insensitivity; the implementation
    /// was exact-match. Pin the promised behavior.
    #[test]
    fn find_tool_is_case_insensitive() {
        assert_eq!(find_tool("AH").map(|t| t.name), Some("ah"));
        assert_eq!(find_tool("Espectacular").map(|t| t.name), Some("ah"));
        assert_eq!(find_tool("WAI").map(|t| t.name), Some("wai"));
    }

    /// 'ddl'/'dulce'/'dulce-de-leche' refer to ddl itself — agents typed
    /// them into `ddl install` after 404ing on charly-vibes/ddl (finanzas
    /// 9/29, REPLy 10/1). They must be recognized, not "unknown".
    #[test]
    fn self_names_are_recognized() {
        for name in ["ddl", "dulce", "dulce-de-leche", "DDL", "Dulce"] {
            assert!(is_self_name(name), "{name} should resolve to ddl itself");
        }
        assert!(!is_self_name("wai"));
        assert!(!is_self_name(""));
    }

    /// The self-name error is the teachable moment: it must name the
    /// canonical repo so agents stop guessing `charly-vibes/ddl`.
    #[test]
    fn resolve_tool_self_names_explain() {
        for name in ["ddl", "dulce", "dulce-de-leche", "DULCE"] {
            let err = resolve_tool(name).unwrap_err();
            assert!(
                err.contains("charly-vibes/dulce-de-leche"),
                "error for {name} must name the canonical repo: {err}"
            );
        }
    }

    #[test]
    fn resolve_tool_unknown_suggests_closest() {
        let err = resolve_tool("waii").unwrap_err();
        assert!(err.contains("Did you mean"), "got: {err}");
    }

    #[test]
    fn resolve_tool_accepts_aliases_and_case() {
        assert_eq!(resolve_tool("espectacular").unwrap().name, "ah");
        assert_eq!(resolve_tool("ESPECTACULAR").unwrap().name, "ah");
    }
}
