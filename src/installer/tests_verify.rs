//! Checksum, provenance-tag, and GitHub-token tests.

use super::*;
use crate::dot_ddl::DdlDir;
use crate::error::{DdlError, Result};
use crate::manifest::{InstallLock, LockEntry, Manifest, ToolEntry};
use crate::platform::{MANAGED_TOOLS, Os, PackageManager, Platform, Tool};
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(test)]
mod tests_verify {
    use super::*;
    #[test]
    fn test_release_tag_from_url() {
        assert_eq!(
            release_tag_from_url(
                "https://github.com/charly-vibes/dulce-de-leche/releases/download/v0.8.0/ddl_0.8.0_linux_amd64.tar.gz"
            ),
            Some("v0.8.0")
        );
        assert_eq!(
            release_tag_from_url("https://example.com/nightly/tool.tar.gz"),
            None
        );
    }

    #[test]
    fn test_sha256_hex_known_digests() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"hello world"),
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[test]
    fn test_parse_checksums_shasum_format() {
        let content = concat!(
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855  wai_0.1.0_darwin_arm64.tar.gz\n",
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9  beads_1.0.0_linux_amd64.zip\n"
        );
        let map = parse_checksums(content).expect("should parse two lines");
        assert_eq!(
            map.get("wai_0.1.0_darwin_arm64.tar.gz").unwrap(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            map.get("beads_1.0.0_linux_amd64.zip").unwrap(),
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[test]
    fn test_parse_checksums_single_space_separator() {
        let content =
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9 wai.tar.gz\n";
        let map = parse_checksums(content).expect("single-space lines parse");
        assert_eq!(
            map.get("wai.tar.gz").unwrap(),
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[test]
    fn test_parse_checksums_binary_mode_star_prefix() {
        // `sha256sum -b` emits `hex *filename` (single space, asterisk prefix)
        let content =
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9 *wai.tar.gz\n";
        let map = parse_checksums(content).expect("binary-mode line parses");
        assert_eq!(
            map.get("wai.tar.gz").unwrap(),
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[test]
    fn test_parse_checksums_skips_malformed_lines() {
        let content = concat!(
            "not-a-checksum oops.tar.gz\n",
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9  good.tar.gz\n",
            "\n",
            "\"b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9\" *quoted.tar.gz\n"
        );
        let map = parse_checksums(content).expect("at least one valid line");
        assert_eq!(map.len(), 2);
        assert!(map.contains_key("good.tar.gz"));
        assert!(
            map.contains_key("quoted.tar.gz"),
            "bsd-style quoted line parses"
        );
    }

    #[test]
    fn test_parse_checksums_no_valid_lines_returns_none() {
        assert_eq!(parse_checksums(""), None);
        assert_eq!(parse_checksums("garbage\nlines\n"), None);
    }

    #[test]
    fn test_verify_checksum_accepts_matching_digest() {
        let digest = sha256_hex(b"payload");
        assert!(verify_checksum(b"payload", &digest, "wai", "https://example.test").is_ok());
    }

    #[test]
    fn test_verify_checksum_aborts_on_mismatch_naming_tool() {
        let digest = sha256_hex(b"payload");
        let err = verify_checksum(
            b"tampered",
            &digest,
            "wai",
            "https://example.test/asset.tar.gz",
        )
        .expect_err("mismatch must error");
        match &err {
            DdlError::ChecksumMismatch { tool, .. } => assert_eq!(tool, "wai"),
            other => panic!("expected ChecksumMismatch, got: {other}"),
        }
        // Error text names the tool and both digests so the operator can triage
        let text = err.to_string();
        assert!(text.contains("wai"));
        assert!(text.contains(&digest));
    }

    #[test]
    fn test_verify_checksum_accepts_uppercase_expected() {
        // Some release pipelines emit uppercase hex — normalize before comparing
        let digest = sha256_hex(b"payload").to_uppercase();
        assert!(verify_checksum(b"payload", &digest, "wai", "https://example.test").is_ok());
    }

    /// github_token reads GITHUB_TOKEN, then GH_TOKEN, and ignores empty
    /// values. Env vars are process-global, so the test serializes access
    /// itself and restores prior values afterwards.
    #[test]
    fn test_github_token_reads_env() {
        // SAFETY: env var manipulation in a single-threaded test; values are
        // saved and restored so other tests see the original environment.
        unsafe {
            let saved_gh = std::env::var("GITHUB_TOKEN").ok();
            let saved_github = std::env::var("GH_TOKEN").ok();

            std::env::remove_var("GITHUB_TOKEN");
            std::env::remove_var("GH_TOKEN");
            assert_eq!(github_token(), None);

            std::env::set_var("GITHUB_TOKEN", "tok-1");
            assert_eq!(github_token(), Some("tok-1".to_string()));

            // Empty GITHUB_TOKEN is ignored, falls through to GH_TOKEN
            std::env::set_var("GITHUB_TOKEN", "");
            std::env::set_var("GH_TOKEN", "tok-2");
            assert_eq!(github_token(), Some("tok-2".to_string()));

            match saved_gh {
                Some(v) => std::env::set_var("GITHUB_TOKEN", v),
                None => std::env::remove_var("GITHUB_TOKEN"),
            }
            match saved_github {
                Some(v) => std::env::set_var("GH_TOKEN", v),
                None => std::env::remove_var("GH_TOKEN"),
            }
        }
    }
}
