# Development

## Prerequisites

- [Rust](https://rustup.rs) (1.75+)
- [just](https://github.com/casey/just) — command runner

## Setup

```bash
just setup
```

## Common commands

| Command | Description |
|---------|-------------|
| `just` | Build and test |
| `just build` | Build debug binary |
| `just build-release` | Build release binary |
| `just test` | Run all tests |
| `just lint` | Run clippy linter |
| `just fmt` | Format code |
| `just ci` | Full CI pipeline (fmt + lint + test + build) |
| `just run <args>` | Run ddl with arguments |
| `just docs` | Build documentation locally |
| `just docs-serve` | Live preview docs at localhost:3000 |

## CI/CD

CI runs on GitHub Actions with the same `just ci` command used locally.

- **CI**: runs on every push/PR to main
- **Release**: triggered by git tags `v*` — builds cross-platform binaries,
  publishes to GitHub Releases, crates.io, Homebrew, and Scoop
- **Docs**: builds and deploys mdBook to GitHub Pages on pushes to `docs/`

## Release process

```bash
# 1. Update version in Cargo.toml
# 2. Commit and push
git add Cargo.toml
git commit -m "Release v0.1.0"
git push

# 3. Tag and push
git tag v0.1.0
git push --tags

# 4. GitHub Actions handles the rest:
#    - Builds binaries for all platforms
#    - Creates GitHub Release
#    - Publishes to crates.io
#    - Updates Homebrew formula
#    - Updates Scoop manifest
```

## Repository structure

```
.github/workflows/
  ci.yml          # CI workflow
  release.yml     # Cross-platform release workflow
  docs.yml        # Documentation deployment

docs/
  book.toml       # mdBook configuration
  src/            # Documentation source files

scripts/
  update-homebrew.py  # Homebrew formula updater
  update-scoop.py     # Scoop manifest updater

src/
  main.rs         # CLI entry point
  lib.rs          # Library root
  cli.rs          # Command structure (clap derive)
  error.rs        # Error types
  manifest.rs     # Manifest management
  platform.rs     # Platform detection
```