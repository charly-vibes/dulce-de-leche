# dulce-de-leche — dev commands
#
# Same commands run locally and in CI for consistent diagnostics.
# Run `just` for default (build + test), `just ci` for full pipeline.

set shell := ["bash", "-uc"]

# Default: build and test
default: build test

# === Build Commands ===

# Build debug binary
build:
    cargo build

# Build release binary (optimized)
build-release:
    cargo build --release

# Run with arguments (e.g., `just run init`, `just run status`)
run *args:
    cargo run -- {{args}}

# Install locally to ~/.cargo/bin
install:
    cargo install --path .

# === Test Commands ===


# TDD loop (most obvious entry, DDL test-runner standard — see DDL-85p in
# the dulce-de-leche repo): testaruda exec — select the tests the working
# diff affects, run only those, ingest, calibrate. Tolerance contract:
# exit 0/10/20 green; NOT CALIBRATED with 0 actual failures is a vacuous
# pass (tambor-cpd policy), with failures it blocks. Local store
# (.testaruda/, gitignored) initializes on first use; a stale store
# self-heals (wild-z56).
test:
    #!/usr/bin/env bash
    set -uo pipefail
    if [ ! -d .testaruda ]; then
      testaruda init || { echo "❌ testaruda init failed" >&2; exit 1; }
    fi
    code=0
    testaruda exec || code=$?
    case "$code" in
      0|10|20) exit 0 ;;
      4)
        echo "⚠️ runner exit 4 — pruning stale local calibration store and retrying once (wild-z56)" >&2
        rm -rf .testaruda
        testaruda init || { echo "❌ testaruda re-init failed" >&2; exit 1; }
        testaruda exec ;;
      *) exit "$code" ;;
    esac

# Blast-radius preview: which tests a change WOULD affect — runs nothing.
# Useful before a refactor; optionally name files: just test-blast src/foo.rs
test-blast *files:
    #!/usr/bin/env bash
    if [ -z "{{files}}" ]; then
      testaruda select --pre-edit --human
    else
      testaruda select --pre-edit --human --files "{{files}}"
    fi

test-full:
    cargo test

# Validate spec-test correspondence (requires ah/espectacular)
validate:
    @echo "ah check — requires ah to be installed"

# Run tests with output
test-verbose:
    cargo test -- --nocapture

# Run a specific test
test-one name:
    cargo test {{name}} -- --nocapture

# === Lint Commands ===

# Run clippy linter
lint:
    cargo clippy -- -D warnings

# Format all Rust files
fmt:
    cargo fmt

# Check formatting (no changes)
fmt-check:
    cargo fmt -- --check

# === CI Commands ===

# Executable fixture tests for the install.sh bootstrap (DDL-1ay / SEC-03):
# tampered/missing checksums must install nothing; clean fixture must install.
bootstrap-fixtures:
    bash tests/install_sh_fixtures.sh

# Full CI pipeline
ci: fmt-check lint test bootstrap-fixtures build-release
    @echo "✅ CI pipeline passed"

# Dependency advisory audit (RustSec). Used by the release workflow's
# security gate; publication is blocked when it fails.
advisory:
    cargo audit --version >/dev/null 2>&1 || cargo install --locked cargo-audit
    cargo audit

# Secret scan over tracked files. Used by the release workflow's security
# gate; publication is blocked when a potential secret is found.
secret-scan:
    #!/usr/bin/env bash
    set -euo pipefail
    if git grep -nIE '(AKIA[0-9A-Z]{16}|ghp_[A-Za-z0-9]{36}|github_pat_[A-Za-z0-9_]{22,}|-----BEGIN [A-Z ]*PRIVATE KEY-----)' -- . 2>/dev/null; then
        echo "❌ potential secret detected in tracked files (see matches above)" >&2
        exit 1
    fi
    echo "✅ no potential secrets in tracked files"

# Pre-push checks (fast gate)
pre-push: fmt-check lint test
    @echo "✅ Pre-push checks passed"

# === Setup Commands ===

# Setup development environment
setup:
    @echo "Checking Rust installation..."
    rustc --version
    cargo --version
    @echo ""
    @echo "Installing dev tools..."
    rustup component add clippy rustfmt
    @echo ""
    @echo "✅ Development environment ready"
    @echo "Run 'just test' to verify setup"

# === Docs Commands ===

# Build docs locally (requires mdbook)
docs:
    mdbook build

# Live preview docs at localhost:3000 (requires mdbook)
docs-serve:
    mdbook serve

# === Design Commands ===

# Show design doc
design:
    @bat docs/design.md 2>/dev/null || less docs/design.md

# Show ecosystem map
ecosystem:
    @bat docs/ecosystem-map.md 2>/dev/null || less docs/ecosystem-map.md

# List all managed tools
tools:
    @echo "Rust tools:        wai, dont, ah, pretender, testaruda, vampiro, specodelic"
    @echo "Non-Rust tools:    bd (Go), openspec, incitaciones (npm)"
    @echo "Install-only:      bd, openspec, incitaciones, turu"
    @echo ""
    @echo "See docs/ecosystem-map.md for details"

# Run the Rule of 5 review on a document
# Usage: just review path/to/document.md
review path:
    @echo "Use the rule-of-5-universal skill to review: $(path)"

# Check project file structure
tree:
    @find . -not -path './.git/*' -not -path './target/*' -not -name '*.git' | sort | head -50

# Generate a fresh ecosystem survey from the charly-vibes monorepo
survey:
    @echo "Run from the charly-vibes root:"
    @echo "  find . -name Cargo.toml -maxdepth 3 | sort"
    @echo "  for f in \$(find . -name Cargo.toml -maxdepth 3); do"
    @echo "    grep '^name\\|^description' \"\$$f\""
    @echo "  done"

# === Utility Commands ===

# Clean build artifacts
clean:
    cargo clean

# Check without building (faster feedback)
check:
    cargo check

# Show dependency tree
deps:
    cargo tree

# Update dependencies
update:
    cargo update

# Show available commands
help:
    @just --list