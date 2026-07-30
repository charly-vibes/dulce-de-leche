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

# Run all tests
test:
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

# Full CI pipeline
ci: fmt-check lint test build-release
    @echo "✅ CI pipeline passed"

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
    mdbook build docs

# Live preview docs at localhost:3000 (requires mdbook)
docs-serve:
    mdbook serve docs

# === Design Commands ===

# Show design doc
design:
    @bat docs/design.md 2>/dev/null || less docs/design.md

# Show ecosystem map
ecosystem:
    @bat docs/ecosystem-map.md 2>/dev/null || less docs/ecosystem-map.md

# List all managed tools
tools:
    @echo "Active Rust tools: wai, dont, ah, pretender, testaruda, fotos-mcp"
    @echo "Non-Rust tools:    fabbro, fotos"
    @echo "In spec:           vampiro"
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