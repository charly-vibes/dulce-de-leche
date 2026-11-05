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
