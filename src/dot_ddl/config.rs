//! DdlConfig file, gitignore entries, and legacy-config registry.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Gate command lines rendered into the lefthook managed block (DDL-1bz).
///
/// Every field is optional: absent (`None`) uses the shipped default, and
/// `Some("")` omits that command from the rendered block entirely.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GatesConfig {
    /// Command run by the `ddl-ah-check` hook entry (default `ah check`).
    #[serde(default)]
    pub ah_check: Option<String>,
    /// Command run by the `ddl-pretender-gate` hook entry.
    #[serde(default)]
    pub pretender_gate: Option<String>,
    /// Command run by the `ddl-spk-lint` hook entry (empty omits the entry).
    #[serde(default)]
    pub spk_lint: Option<String>,
}

/// ddl's own configuration, written to `.ddl/config.toml`.
///
/// Implements `genesis::config::ConfigFile` for standard read/write/validate.

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DdlConfig {
    /// Installation source preference: "auto", "binary", "cargo", "brew", "scoop"
    #[serde(default = "default_install_source")]
    pub install_source: String,
    /// Automatically upgrade tools when running ddl init
    #[serde(default)]
    pub auto_upgrade: bool,
    /// Strategy for .gitignore entries: "auto", "prompt", "skip"
    #[serde(default = "default_gitignore_strategy")]
    pub gitignore_strategy: String,
    /// Command overrides for the managed lefthook gate blocks.
    #[serde(default)]
    pub gates: GatesConfig,
}

fn default_install_source() -> String {
    "auto".to_string()
}

fn default_gitignore_strategy() -> String {
    "prompt".to_string()
}

impl Default for DdlConfig {
    fn default() -> Self {
        Self {
            install_source: "auto".to_string(),
            auto_upgrade: false,
            gitignore_strategy: "prompt".to_string(),
            gates: GatesConfig::default(),
        }
    }
}

impl genesis::config::ConfigFile for DdlConfig {
    /// The config lives at `<ddl_dir>/config.toml`.
    ///
    /// Although the genesis `ConfigFile` trait calls this `repo_root`,
    /// ddl passes its `.ddl/` directory path here (not the project root).
    fn path(repo_root: &Path) -> PathBuf {
        repo_root.join("config.toml")
    }

    fn validate(
        &self,
    ) -> std::result::Result<Vec<genesis::config::ConfigValidation>, genesis::config::ConfigError>
    {
        let mut results = Vec::new();
        match self.install_source.as_str() {
            "auto" | "binary" | "cargo" | "brew" | "scoop" => {}
            other => results.push(genesis::config::ConfigValidation::warning(
                "install_source",
                format!("unknown install source '{}', expected one of: auto, binary, cargo, brew, scoop", other),
            )),
        }
        match self.gitignore_strategy.as_str() {
            "auto" | "prompt" | "skip" => {}
            other => results.push(genesis::config::ConfigValidation::warning(
                "gitignore_strategy",
                format!(
                    "unknown gitignore strategy '{}', expected one of: auto, prompt, skip",
                    other
                ),
            )),
        }
        for (field, value) in [
            ("gates.ah_check", &self.gates.ah_check),
            ("gates.pretender_gate", &self.gates.pretender_gate),
            ("gates.spk_lint", &self.gates.spk_lint),
        ] {
            if let Some(value) = value
                && value.contains('\n')
            {
                results.push(genesis::config::ConfigValidation::warning(
                    field,
                    format!("{field} must be a single line (a newline would corrupt the managed lefthook block)"),
                ));
            }
        }
        Ok(results)
    }
}

/// The .gitignore entries to add for .ddl/ data files.
pub const GITIGNORE_ENTRIES: &str = r##"
# dulce-de-leche — data files (do not commit)
.ddl/**/*.db
.ddl/**/store/
.ddl/install-log.json
.ddl/doctor-cache.json
.ddl/compatibility-cache.json
"##;

/// Legacy tool config directories mapped to their binary names.
/// Used by `ddl migrate` to detect existing configs.
pub(crate) const LEGACY_CONFIGS: &[(&str, &str)] = &[
    ("wai", ".wai"),
    ("dont", ".dont"),
    ("ah", ".espectacular"),
    ("pretender", ".pretender.toml"),
    ("testaruda", ".testaruda"),
    ("vampiro", ".vampiro"),
    ("specodelic", ".specs"),
    ("funkalization", ".funkalization"),
    // Note: bd, openspec, incitaciones, and turu are intentionally absent —
    // they are install-only (bd manages repo-local `.beads/` itself, openspec
    // manages `openspec/` itself, skills land in `~/.agents/skills/`, and
    // turu's state is home-dir global). See docs/ecosystem-map.md.
];

/// The legacy tool config directories, as `(binary name, legacy path)` pairs.
/// Tools without a legacy config (e.g. `bd`, `openspec`) are absent.
pub fn legacy_configs() -> &'static [(&'static str, &'static str)] {
    LEGACY_CONFIGS
}
