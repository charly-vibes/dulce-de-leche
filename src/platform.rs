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

/// A managed tool in the charly-vibes ecosystem.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    pub name: &'static str,
    pub description: &'static str,
    pub crate_name: &'static str,
    pub formula_name: &'static str,
    pub repo: &'static str,
}

/// All managed tools.
pub const MANAGED_TOOLS: &[Tool] = &[
    Tool {
        name: "wai",
        description: "Workflow manager for AI-driven development",
        crate_name: "wai-cli",
        formula_name: "wai",
        repo: "charly-vibes/wai",
    },
    Tool {
        name: "dont",
        description: "Epistemic discipline for AI-driven development",
        crate_name: "dont-cli",
        formula_name: "dont",
        repo: "charly-vibes/dont",
    },
    Tool {
        name: "ah",
        description: "Behavioral specification testing",
        crate_name: "espectacular",
        formula_name: "ah",
        repo: "charly-vibes/espectacular",
    },
    Tool {
        name: "pretender",
        description: "Code quality automation",
        crate_name: "pretender",
        formula_name: "pretender",
        repo: "charly-vibes/pretender",
    },
    Tool {
        name: "testaruda",
        description: "Test selection and prioritization",
        crate_name: "testaruda",
        formula_name: "testaruda",
        repo: "charly-vibes/testaruda",
    },
    Tool {
        name: "fotos-mcp",
        description: "Screenshot and image analysis MCP server",
        crate_name: "fotos-mcp",
        formula_name: "fotos-mcp",
        repo: "charly-vibes/fotos",
    },
    Tool {
        name: "fabbro",
        description: "Local-first code review annotation tool",
        crate_name: "fabbro",
        formula_name: "fabbro",
        repo: "charly-vibes/fabbro",
    },
];

/// Find a tool by name (case-insensitive).
pub fn find_tool(name: &str) -> Option<&'static Tool> {
    MANAGED_TOOLS.iter().find(|t| t.name == name || t.crate_name == name)
}

/// Simple "did you mean?" suggestion using genesis SuggestionEngine.
/// Returns the closest matching tool name if within threshold.
pub fn did_you_mean(input: &str, candidates: &[&str]) -> Option<String> {
    use genesis::suggestions::{CommandRegistry, SuggestionEngine};

    let mut registry = CommandRegistry::new();
    registry.register(
        "ddl",
        candidates.iter().map(|c| c.to_string()).collect(),
    );

    let engine = SuggestionEngine::new();
    match engine.suggest_typo(input, &registry) {
        Some(genesis::suggestions::Suggestion::DidYouMean { suggestion, .. }) => {
            Some(suggestion)
        }
        _ => None,
    }
}