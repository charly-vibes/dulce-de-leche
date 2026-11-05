//! Tool name resolution: aliases, self-names, did-you-mean.

use super::*;

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
