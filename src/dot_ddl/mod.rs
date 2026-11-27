//! .ddl/ directory management — creation, migration, symlinks, gitignore.
//!
//! The `.ddl/` directory is the single source of truth for the charly-vibes
//! toolset. It contains:
//!   - manifest.json — installed tool versions, migration state
//!   - config.toml — ddl's own configuration
//!   - <tool>/ — per-tool config directories (symlinks in Phase 1)

mod config;
mod dir;
mod migrate;
// pub(crate) (not private): the repo-status record writer reuses the
// atomic staged-write and manifest-lock primitives (DDL-76lm).
pub(crate) mod persist;

pub use config::*;
pub use dir::*;
pub use migrate::*;
pub(crate) use persist::*;

#[cfg(test)]
mod sec05_tests {
    use super::*;

    /// SEC-05 (DDL-sg8), link-swap coverage: after the swap from a regular
    /// manifest to a link, the no-follow open itself refuses — no prior
    /// check is involved, so the check-then-open race window cannot be used
    /// to get a write through.
    #[test]
    fn no_follow_open_refuses_manifest_swapped_to_symlink() {
        let dir = tempfile();
        let manifest = dir.path().join("manifest.json");
        std::fs::write(&manifest, "{}").unwrap();

        // Regular file: open succeeds (plain save path).
        open_manifest_no_follow(&manifest).unwrap();

        // Swap in a link pointing at an outside file; the open itself must
        // refuse even though no path-based check has run since the swap.
        let outside = dir.path().join("outside.json");
        std::fs::write(&outside, "harmless").unwrap();
        #[cfg(unix)]
        {
            std::fs::remove_file(&manifest).unwrap();
            std::os::unix::fs::symlink(&outside, &manifest).unwrap();
        }
        #[cfg(windows)]
        {
            std::fs::remove_file(&manifest).unwrap();
            std::os::windows::fs::symlink_file(&outside, &manifest)
                .expect("symlink_file requires admin/dev-mode privileges");
        }

        let err = open_manifest_no_follow(&manifest).unwrap_err();
        assert!(err.to_string().contains("symlink"), "got: {err}");
        // The outside file must be untouched.
        assert_eq!(std::fs::read_to_string(&outside).unwrap(), "harmless");
    }

    /// SEC-05 (DDL-sg8): a dangling link at the manifest path is refused by
    /// the no-follow open, and the dangling target is not created.
    #[cfg(unix)]
    #[test]
    fn no_follow_open_refuses_dangling_link() {
        let dir = tempfile();
        let manifest = dir.path().join("manifest.json");
        std::os::unix::fs::symlink(dir.path().join("nowhere.json"), &manifest).unwrap();

        let err = open_manifest_no_follow(&manifest).unwrap_err();
        assert!(err.to_string().contains("symlink"), "got: {err}");
        assert!(!dir.path().join("nowhere.json").exists());
    }

    /// SEC-05 (DDL-sg8): the no-follow open must not have regressed to
    /// following links — a link to an in-project file is still refused,
    /// because any manifest link can be swapped between check and write.
    #[cfg(unix)]
    #[test]
    fn no_follow_open_refuses_even_in_project_link() {
        let dir = tempfile();
        let manifest = dir.path().join("manifest.json");
        let inside = dir.path().join("inside.json");
        std::fs::write(&inside, "keep").unwrap();
        std::os::unix::fs::symlink(&inside, &manifest).unwrap();

        assert!(open_manifest_no_follow(&manifest).is_err());
        assert_eq!(std::fs::read_to_string(&inside).unwrap(), "keep");
    }

    fn tempfile() -> tempfile::TempDir {
        tempfile::Builder::new()
            .prefix("ddl-sec05")
            .tempdir()
            .expect("tempdir")
    }
}
