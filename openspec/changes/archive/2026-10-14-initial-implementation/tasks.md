## 1. Project Scaffolding
- [ ] 1.1 Create Rust crate with `cargo init`
- [ ] 1.2 Add dependencies: clap (derive), miette (fancy), serde, serde_json, toml, genesis-vibes, chrono, reqwest (blocking, rustls-tls), dirs
- [ ] 1.3 Set up CLI structure with clap derive (`src/cli.rs`)
- [ ] 1.4 Set up error types with miette (`src/error.rs`)
- [ ] 1.5 Create platform detection module (`src/platform.rs`)
- [ ] 1.6 Create manifest management module (`src/manifest.rs`)
- [ ] 1.7 Set up `--json` output via genesis-vibes envelope
- [ ] 1.8 Set up cross-compilation CI (GitHub Actions release workflow)
- [ ] 1.9 Write tests: CLI parsing, help text, exit codes

## 2. Platform Detection
- [ ] 2.1 Detect OS (macos, linux, windows)
- [ ] 2.2 Detect architecture (arm64, amd64)
- [ ] 2.3 Detect available package managers (brew, cargo, scoop)
- [ ] 2.4 Detect if tool is already installed (check PATH)
- [ ] 2.5 Write tests: detection for all platform combinations

## 3. Installation Strategy
- [ ] 3.1 Implement fallback chain: binary → cargo → brew/scoop
- [ ] 3.2 Implement brew installer (macOS)
- [ ] 3.3 Implement cargo installer (any platform with Rust)
- [ ] 3.4 Implement binary download installer (any platform, no prerequisites)
- [ ] 3.5 Implement scoop installer (Windows)
- [ ] 3.6 Implement placeholder formula detection (detect 0.0.0 versions, skip brew)
- [ ] 3.7 Implement tool init runner (run each tool's init after install)
- [ ] 3.8 Write tests: install success, install failure, partial failure, fallback chain

## 4. Init Command
- [ ] 4.1 Implement `ddl init` — interactive mode
- [ ] 4.2 Implement `ddl init --yes` — non-interactive mode
- [ ] 4.3 Implement `ddl init --tools wai,dont` — selective install
- [ ] 4.4 Implement `ddl init` — already initialized detection
- [ ] 4.5 Write tests: init all, init selective, init repeated

## 5. Install Command
- [ ] 5.1 Implement `ddl install <tool>` — single tool install
- [ ] 5.2 Implement "did you mean" suggestions for unknown tool names
- [ ] 5.3 Write tests: install known tool, install unknown tool, reinstall

## 6. Status Command
- [ ] 6.1 Implement `ddl status` — cross-tool health overview
- [ ] 6.2 Implement subprocess calls to each tool's status command
- [ ] 6.3 Implement JSON output parsing for tool status
- [ ] 6.4 Implement fallback to human output parsing
- [ ] 6.5 Implement `ddl status --json`
- [ ] 6.6 Write tests: all healthy, partial healthy, no tools installed

## 7. Doctor Command
- [ ] 7.1 Implement `ddl doctor` — cross-tool diagnostics
- [ ] 7.2 Implement `ddl doctor --fix` — auto-fix mode
- [ ] 7.3 Implement `ddl doctor --json`
- [ ] 7.4 Write tests: doctor all, doctor with fixes

## 8. Version Management
- [ ] 8.1 Implement manifest read/write (`src/manifest.rs`)
- [ ] 8.2 Implement version compatibility matrix (embedded in binary)
- [ ] 8.3 Implement `ddl version` command
- [ ] 8.4 Implement version check against latest available
- [ ] 8.5 Write tests: manifest creation, version check, compatibility validation

## 9. Upgrade Command
- [ ] 9.1 Implement `ddl upgrade` — update all tools
- [ ] 9.2 Implement `ddl upgrade <tool>` — update single tool
- [ ] 9.3 Implement upgrade via platform package manager
- [ ] 9.4 Write tests: upgrade all, upgrade single, upgrade fails

## 10. .ddl/ Directory
- [ ] 10.1 Implement `.ddl/` directory creation in init
- [ ] 10.2 Implement `.gitignore` template creation
- [ ] 10.3 Implement `.ddl/config.toml` creation
- [ ] 10.4 Implement Phase 1 symlink creation (move + symlink)
- [ ] 10.5 Implement Windows symlink fallback (directory junctions)
- [ ] 10.6 Implement broken symlink detection in doctor
- [ ] 10.7 Implement file locking for manifest writes (fs2 crate)
- [ ] 10.8 Write tests: directory structure, gitignore creation, symlink creation, manifest locking

## 11. Cross-Platform Binary Distribution
- [ ] 11.1 Set up GitHub Actions release workflow
- [ ] 11.2 Cross-compile for macos-arm64, macos-amd64, linux-arm64, linux-amd64, windows-amd64
- [ ] 11.3 Upload binaries to GitHub releases
- [ ] 11.4 Add Homebrew formula to `homebrew-charly` tap
- [ ] 11.5 Add Scoop package to `scoop-charly` bucket
- [ ] 11.6 Write test: binary runs on all platforms (CI matrix)

## 12. Compatibility Matrix Hosting
- [ ] 12.1 Create `compatibility.json` at `charly-vibes.github.io/ddl/compatibility.json`
- [ ] 12.2 Implement matrix fetch at runtime (reqwest)
- [ ] 12.3 Implement embedded fallback matrix
- [ ] 12.4 Implement matrix caching in `.ddl/compatibility-cache.json`
- [ ] 12.5 Set up CI to update compatibility.json when a tool releases
- [ ] 12.6 Write tests: matrix fetch, fallback, cache invalidation

## 13. Windows Support
- [ ] 13.1 Implement binary download on Windows (`.exe` extension, PATH handling)
- [ ] 13.2 Implement Scoop-not-found fallback (binary download)
- [ ] 13.3 Implement directory junctions instead of symlinks
- [ ] 13.4 Write tests: Windows binary download, Scoop fallback, junction creation

## 14. Network Resilience
- [ ] 14.1 Implement per-tool status tracking in manifest (`"status": "installed" | "failed"`)
- [ ] 14.2 Implement retry for failed tools on subsequent `ddl init` / `ddl install`
- [ ] 14.3 Implement 404 handling for binary downloads (fall back to error, not cargo)
- [ ] 14.4 Write tests: network failure, retry, 404 handling

## 15. Documentation
- [ ] 15.1 Write README.md with quick start, examples, and platform guide
- [ ] 15.2 Write `--help` text for all commands
- [ ] 15.3 Add managed block for wai integration (AGENTS.md)
- [ ] 15.4 Write CLAUDE.md with agent context