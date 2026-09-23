# Troubleshooting

## `ddl init` fails with "Prerequisite missing"

ddl checks for prerequisites before installing. If a requirement is missing,
install it and try again:

- **Binary download:** needs `curl` or `wget` (pre-installed on most systems)
- **Cargo install:** needs `rustc` + `cargo` — install via [rustup](https://rustup.rs)
- **incitaciones (npm):** needs `npm` — install via [nodejs.org](https://nodejs.org)

Homebrew and Scoop are not used by ddl for installs — the tap and bucket
exist only for manual installs of the tools themselves.

## `ddl install` fails with "Tool not found"

Check available tools:
- `wai` — Workflow manager
- `dont` — Epistemic discipline
- `ah` — Behavioral specification testing
- `pretender` — Code quality
- `testaruda` — Test selection
- `vampiro` — Composition checking
- `fotos-mcp` — Screenshot MCP server
- `fabbro` — Code review annotations
- `incitaciones` — Prompts and skills for CLI LLM tools (npm)
- `turu` — Knowledge workspace management (whisper-vibes)

## `ddl status` shows no tools

Run `ddl init` first to install tools. If you already have tools installed
manually, run `ddl init --no-install` to create the manifest.

## Binary download returns 404

The binary for your platform may not be published yet. Try:
```bash
cargo install dulce-de-leche
```

## Network is unavailable

ddl works offline for already-installed tools:
- `ddl status` works offline
- `ddl version` works offline (without `--check`)
- `ddl upgrade` requires network access