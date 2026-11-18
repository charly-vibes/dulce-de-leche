//! Tool registry types (Tool, ToolCategory, Maturity) and MANAGED_TOOLS catalog.

use serde::{Deserialize, Serialize};

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
        name: "sad",
        description: "Documentation gate — code should speak for itself; prose means refactor",
        // crates.io `sad` is owned by an unrelated 2021 placeholder project,
        // so there is no cargo-install fallback; sad ships release binaries.
        crate_name: "",
        formula_name: "sad",
        repo: "charly-vibes/sad",
        npm_package: None,
        category: ToolCategory::Extension,
        maturity: Maturity::Experimental,
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
    Tool {
        name: "funkalization",
        description: "Visual semantic design workspace — versioned design semantics, obligations, scoped evidence, loss-reporting adapters",
        crate_name: "funkalization",
        formula_name: "funkalization",
        repo: "charly-vibes/funkalization",
        npm_package: None,
        category: ToolCategory::Extension,
        maturity: Maturity::Experimental,
    },
    Tool {
        name: "wild",
        description: "Contract-based compatibility checking for software updates",
        crate_name: "wild-vibes",
        formula_name: "wild",
        repo: "charly-vibes/wild",
        npm_package: None,
        category: ToolCategory::Extension,
        maturity: Maturity::Experimental,
    },
    Tool {
        name: "bajan",
        description: "Spec-driven knowledge-graph pipeline CLI — ingest sources, extract claims, human-gated adoption and entity review",
        crate_name: "bajan",
        formula_name: "bajan",
        repo: "charly-vibes/bajan",
        npm_package: None,
        category: ToolCategory::Extension,
        maturity: Maturity::Experimental,
    },
    Tool {
        name: "podras",
        description: "Per-repository project-status brief for humans and AI agents — fixed-section brief, focus/park verbs, typed agent envelope",
        crate_name: "podras",
        formula_name: "podras",
        repo: "charly-vibes/podras",
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
