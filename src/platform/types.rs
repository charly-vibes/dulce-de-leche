//! Platform, Os, Arch, PackageManager types and detection.

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
        Self::from_os_str(std::env::consts::OS)
    }

    /// Map a raw OS string to [`Os`]; unsupported platforms (32-bit,
    /// FreeBSD, …) return `None` (bootstrap/unsupported-platform).
    pub fn from_os_str(os: &str) -> Option<Self> {
        match os {
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
        Self::from_arch_str(std::env::consts::ARCH)
    }

    /// Map a raw architecture string to [`Arch`]; unsupported
    /// architectures return `None`.
    pub fn from_arch_str(arch: &str) -> Option<Self> {
        match arch {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_os_str_covers_all_supported_platforms() {
        assert_eq!(Os::from_os_str("macos"), Some(Os::Macos));
        assert_eq!(Os::from_os_str("linux"), Some(Os::Linux));
        assert_eq!(Os::from_os_str("windows"), Some(Os::Windows));
        // 32-bit / FreeBSD / unknown → unsupported, no Platform can be built.
        assert_eq!(Os::from_os_str("freebsd"), None);
        assert_eq!(Os::from_os_str(""), None);
    }

    #[test]
    fn test_arch_str_covers_amd64_and_arm64_only() {
        assert_eq!(Arch::from_arch_str("x86_64"), Some(Arch::Amd64));
        assert_eq!(Arch::from_arch_str("aarch64"), Some(Arch::Arm64));
        assert_eq!(Arch::from_arch_str("armv7"), None, "32-bit is unsupported");
        assert_eq!(Arch::from_arch_str("riscv64"), None);
    }

    #[test]
    fn test_platform_naming_matrix_matches_release_asset_keys() {
        // Drift note (DDL-oymq): the contracts text says "macos-*"; the
        // shipped convention (release.yml asset keys, download URLs) is
        // darwin/windows/linux + amd64/arm64. This pins the actual names.
        let p = |os: Os, arch: Arch| Platform { os, arch }.as_str();
        assert_eq!(p(Os::Macos, Arch::Arm64), "darwin-arm64");
        assert_eq!(p(Os::Macos, Arch::Amd64), "darwin-amd64");
        assert_eq!(p(Os::Linux, Arch::Arm64), "linux-arm64");
        assert_eq!(p(Os::Linux, Arch::Amd64), "linux-amd64");
        assert_eq!(p(Os::Windows, Arch::Amd64), "windows-amd64");
    }

    #[test]
    fn test_detect_succeeds_on_this_host() {
        // This repo's supported hosts; the release targets above are the
        // matrix this pins.
        assert!(
            Platform::detect().is_some(),
            "host platform must be detectable"
        );
    }
}
