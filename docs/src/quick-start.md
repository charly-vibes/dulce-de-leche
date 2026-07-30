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
✓ brew install wai ah dont pretender testaruda fotos-mcp
✓ created .ddl/ with configs for all tools
✓ ran wai init, dont prime, ah init, pretender init, testaruda init
✓ created .gitignore entries for .ddl/ data files
```

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