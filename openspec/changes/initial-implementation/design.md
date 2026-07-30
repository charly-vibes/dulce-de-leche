## Context

dulce-de-leche is a new Rust CLI that bootstraps and orchestrates the charly-vibes tool ecosystem. It must work on macOS, Linux, and Windows with no prerequisites beyond the binary itself (on supported platforms).

Key architectural constraints:
- Static binary for Linux deployment (no glibc dependencies)
- Subprocess communication with managed tools (no shared library linking)
- Must work offline for already-installed tools
- Must handle partial failures gracefully
- Must detect and skip placeholder Homebrew formulas

## Goals / Non-Goals

### Goals
- Single binary that runs on all platforms without dependencies
- `ddl init` installs and configures the whole toolset in one command
- `ddl status` shows health of all tools at a glance
- Version pinning for reproducible environments
- Graceful degradation when tools or package managers are unavailable

### Non-Goals
- Reimplementing package resolution (delegate to brew/cargo/scoop)
- Reimplementing tool commands (each tool keeps its own CLI)
- Deep genesis integration via shared library linking (use subprocess protocol)
- Merging tool databases (Cozo, SQLite stay independent)

## Decisions

### Decision 1: Subprocess protocol over genesis linking
**Choice:** ddl communicates with each tool via subprocess calls (`wai status --json`, `dont status --json`).

**Rationale:** If ddl links against genesis, a genesis breaking change requires coordinated releases of ddl + all 6 tools. Subprocess calls decouple release cycles entirely. The cost is slightly slower status checks (subprocess spawn latency ~10ms per tool) but this is acceptable for a non-hot-path operation.

**Alternatives considered:**
- Genesis trait linking: faster but couples release cycles. Rejected.
- No integration (user runs each tool's commands manually): simplest but defeats the purpose. Rejected.

### Decision 2: Which genesis modules are OK to use

ddl depends on genesis-vibes but uses a strict subset of its modules. Only modules that do NOT create shared-library coupling with tools are allowed.

| Genesis module | Status | Rationale |
|---|---|---|
| `genesis::envelope` | ✅ OK | JSON output format — no coupling, just formatting |
| `genesis::suggestions` | ✅ OK | String matching for "did you mean" — no coupling |
| `genesis::scaffold` | ✅ OK | File/directory creation — no coupling |
| `genesis::fixture` | ✅ OK (dev only) | Test helpers — no coupling |
| `genesis::config` | ❌ NO | ConfigFile trait creates coupling with tool configs |
| `genesis::status` | ❌ NO | StatusContributor trait creates coupling with tool state |
| `genesis::doctor` | ❌ NO | DoctorRunner trait creates coupling with tool diagnostics |

### Decision 3: Fallback installation chain
**Choice:** binary download → cargo install → brew/scoop install

**Rationale:** Binary downloads are the most reliable path (no dependencies). Cargo install is the fallback when no binary is available for a platform. Brew/scoop are the last resort because they depend on the user having those package managers installed.

**Important:** Binary download is the ONLY path that requires no prerequisites. On supported platforms (macos-arm64, macos-amd64, linux-arm64, linux-amd64, windows-amd64), ddl guarantees binary availability. Unsupported platforms must use cargo install, which requires the Rust toolchain.

### Decision 4: Static linking for Linux
**Choice:** Use `x86_64-unknown-linux-musl` and `aarch64-unknown-linux-musl` targets for static Linux binaries.

**Rationale:** Eliminates glibc version dependencies. The binary runs on any Linux distro including Alpine (Docker).

### Decision 5: Manifest as JSON
**Choice:** `.ddl/manifest.json` uses JSON (not TOML) for the version manifest.

**Rationale:** The manifest is machine-written and machine-read. JSON is easier to parse programmatically and is the native format for tool subprocess output. TOML is used for human-editable config files.

### Decision 6: Dynamic compatibility matrix
**Choice:** Compatibility matrix is fetched dynamically from `https://charly-vibes.github.io/ddl/compatibility.json`, with an embedded fallback for offline use.

**Rationale:** An embedded-only matrix becomes stale the day after release. Dynamic fetching ensures the matrix is always fresh without requiring a ddl release. The embedded fallback ensures offline operation.

**Alternatives considered:**
- Embedded-only: simple but stale immediately. Rejected.
- No matrix (always permissive): simplest but provides no safety. Rejected.

### Decision 7: No self-update for ddl (v1)
**Choice:** ddl v1 does not self-update. Users upgrade ddl by downloading a new binary or using Homebrew/Scoop.

**Rationale:** Self-update at runtime is error-prone (binary replacement, permissions, Windows file locking). Platform package managers handle this better. If ddl is outdated, `ddl doctor` detects it and suggests upgrading.

**Escalation:** Self-update may be added in v2 if users regularly encounter stale compatibility matrix issues.

## Risks / Trade-offs

### Risk: Version skew between ddl and managed tools
**Mitigation:** Dynamic compatibility matrix. The matrix is fetched at runtime and cached. If the matrix is outdated (e.g., a new tool version was released hours ago), ddl's embedded fallback covers the gap.

### Risk: Subprocess output format changes
**Mitigation:** ddl parses tool subprocess output. If a tool changes its JSON output format, ddl's status parsing breaks. Mitigation: use `serde_json::Value` for lenient parsing instead of strict struct deserialization. Fall back to human-readable output parsing if JSON parsing fails.

### Risk: Binary size
**Mitigation:** ddl pulls in reqwest for binary downloads and clap for CLI. Release builds use LTO and stripping to minimize size. Target: <10MB per binary.

### Risk: 4 of 7 Homebrew formulas are placeholder
**Mitigation:** ddl detects placeholder formulas (version 0.0.0) and falls back to cargo install or binary download.

### Risk: Network failure during install creates half-installed state
**Mitigation:** Manifest records per-tool `status` field (`"installed"`, `"failed"`). Failed installations are retried on next `ddl init` or `ddl install`. See dot-ddl spec for manifest format.

## Open Questions (Resolved)

1. ~~Should ddl also manage version pinning of genesis-vibes itself?~~ **No.** genesis is a library, not a CLI tool. Tool versions are pinned via their respective package managers.

2. ~~Should ddl support a `ddl.lock` file (like Cargo.lock) for stricter version pinning?~~ **No for v1.** The manifest file (`.ddl/manifest.json`) is sufficient. A lockfile may be added in v2 if version drift becomes a problem.

3. ~~Should ddl check for updates to itself?~~ **No for v1.** Self-update is error-prone. `ddl doctor` detects outdated ddl versions and suggests upgrading via the platform package manager or binary re-download.