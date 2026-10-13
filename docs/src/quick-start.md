# Quick Start

## Bootstrap the toolset

```bash
# Install ddl itself (see Installation)
brew tap charly-vibes/charly
brew install dulce-de-leche

# One command to install & configure everything
ddl init
```

## What just happened?

```
✓ detected platform (macOS arm64 / Linux x86_64 / Windows)
✓ installed 10 managed tools — prebuilt release binaries (incitaciones via npm)
✓ created .ddl/ with configs for the config-managed tools
✓ ran each tool's init (wai init, dont prime, ah init, pretender init, …)
✓ checked for incitaciones skills and offered a global install
✓ created .gitignore entries for .ddl/ data files
```

Four tools are **install-only** — they don't keep config under `.ddl/`:
`bd`, `openspec`, `incitaciones`, and `turu`.

## Check the ecosystem

```bash
ddl status
```

Shows for each tool:
- ✓ installed, configured, healthy
- ✗ not installed (suggests: `ddl install wai`)
- ⚠ version outdated (suggests: `ddl upgrade`)

## Install a single tool

```bash
ddl install wai
```

## Update everything

```bash
ddl upgrade
```

## Migrate existing configs

If you already have tools configured with their legacy config directories
(`.wai/`, `.dont/`, etc.), migrate them under `.ddl/`:

```bash
ddl migrate
```

## See what's active

```bash
ddl scope
```

Shows which `.ddl/` is active (walks up from CWD like git).

## Where to go next

- Every command and flag: [Commands](./commands.md)
- Something not working: [Troubleshooting](./troubleshooting.md)
- What the tools do: [Ecosystem Development](./ecosystem.md)

Shows which `.ddl/` is active (walks up from CWD like git).
