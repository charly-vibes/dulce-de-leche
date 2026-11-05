//! Platform detection — OS, architecture, and available package managers.

mod resolve;
mod tool;
mod types;

pub use resolve::*;
pub use tool::*;
pub use types::*;
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
