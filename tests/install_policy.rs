//! Integration tests for the DDL-ei3 install policy: binary-first on every
//! platform, cargo as the runtime fallback, npm only for incitaciones.
//! No network access — method selection and fallback decisions are pure.

use dulce_de_leche::installer::{
    InstallMethod, best_install_method, fallback_after_binary_failure,
};
use dulce_de_leche::platform::{Arch, MANAGED_TOOLS, Os, Platform};

fn all_platforms() -> Vec<Platform> {
    use Arch::{Amd64, Arm64};
    use Os::{Linux, Macos, Windows};
    vec![
        Platform {
            os: Macos,
            arch: Arm64,
        },
        Platform {
            os: Macos,
            arch: Amd64,
        },
        Platform {
            os: Linux,
            arch: Arm64,
        },
        Platform {
            os: Linux,
            arch: Amd64,
        },
        Platform {
            os: Windows,
            arch: Amd64,
        },
    ]
}

#[test]
fn method_matrix_is_binary_first_with_npm_exception() {
    for platform in all_platforms() {
        for tool in MANAGED_TOOLS {
            let method = best_install_method(tool, &platform);
            if tool.npm_package.is_some() {
                assert_eq!(
                    method,
                    InstallMethod::Npm,
                    "npm tool `{}` must install via npm on {}",
                    tool.name,
                    platform.os
                );
            } else {
                assert_eq!(
                    method,
                    InstallMethod::Binary,
                    "tool `{}` must be binary-first on {}",
                    tool.name,
                    platform.os
                );
            }
        }
    }
}

#[test]
fn fallback_only_when_cargo_available() {
    assert_eq!(
        fallback_after_binary_failure(true),
        Some(InstallMethod::Cargo)
    );
    assert_eq!(fallback_after_binary_failure(false), None);
}
