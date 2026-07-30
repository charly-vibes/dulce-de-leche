# Architecture

## Overview

ddl is a thin orchestrator that delegates to existing tools and package
managers. It does not reimplement package resolution, tool commands, or
database management.

## Key design decisions

### Subprocess protocol over shared library linking

ddl communicates with each tool via subprocess calls (`wai status --json`,
`dont status --json`). This decouples release cycles — a genesis breaking
change doesn't require coordinated releases of ddl + all 6 tools.

### Fallback installation chain

Binary download → cargo install → brew/scoop install

Binary downloads are the most reliable path (no dependencies). Cargo install
is the fallback when no binary is available. Brew/scoop are the last resort.

### Phase 1: Symlink farm

Before deep genesis integration, ddl uses a symlink farm:
- `.ddl/wai/` → `.wai/` (symlink)
- `.ddl/dont/` → `.dont/` (symlink)
- etc.

No tool patches needed. Zero risk — symlinks are transparent to file reads.

### Phase 2: Native config proxy

Each tool learns to check `.ddl/<tool>/` first via genesis. Requires
coordinated releases of each tool.

## Directory layout

```
.ddl/
  manifest.json          # {"ddl_version": "1.0.0", "tool_versions": {...}}
  config.toml            # ddl's own config
  compatibility-cache.json  # cached compatibility matrix

  wai/ -> ../.wai/       # symlink (Phase 1)
  dont/ -> ../.dont/     # symlink (Phase 1)
  ah/ -> ../.espectacular/  # symlink (Phase 1)
  pretender.toml -> ../.pretender.toml  # symlink (Phase 1)
  testaruda/ -> ../.testaruda/  # symlink (Phase 1)
```

## Data flow for `ddl status`

```
ddl status
  → reads config from .ddl/config.toml
  → for each registered tool:
      → loads tool config from .ddl/<tool>/
      → runs tool's status command (subprocess)
      → parses output
      → collects result
  → renders aggregate status report
```

## Platform strategy

| Platform | Primary installer | Fallback |
|----------|-------------------|----------|
| macOS (ARM) | Homebrew | Binary download |
| macOS (Intel) | Homebrew | Binary download |
| Linux (ARM) | Binary download | Cargo install |
| Linux (Intel) | Binary download | Cargo install |
| Windows | Scoop | Binary download |

## See also

- [Full design document](https://github.com/charly-vibes/dulce-de-leche/blob/main/docs/design.md)
- [Ecosystem map](https://github.com/charly-vibes/dulce-de-leche/blob/main/docs/ecosystem-map.md)