# Commands

## `ddl init`

Bootstrap the charly-vibes toolset.

```bash
ddl init
# Interactive mode — prompts for tool selection, confirmation

ddl init --yes
# Non-interactive mode — installs all tools, no prompts

ddl init --tools wai,dont
# Selective install — only the specified tools

ddl init --no-install
# Configure only — skip installation, just set up .ddl/
```

## `ddl install <tool>`

Install a single tool by name.

```bash
ddl install wai
ddl install dont
ddl install ah
ddl install pretender
ddl install testaruda
ddl install fotos-mcp
ddl install fabbro
```

## `ddl status`

Show cross-tool health overview.

```bash
ddl status
ddl status --json   # machine-readable output
```

## `ddl doctor`

Run detailed diagnostics across all tools.

```bash
ddl doctor
ddl doctor --fix     # attempt auto-fix
ddl doctor --json    # machine-readable output
```

## `ddl version`

Show versions of ddl and all managed tools.

```bash
ddl version
ddl version --check  # check latest available versions (requires network)
```

## `ddl upgrade`

Update all tools to latest compatible versions.

```bash
ddl upgrade
ddl upgrade wai      # upgrade a single tool
```

## `ddl migrate`

Move existing configs under `.ddl/`.

```bash
ddl migrate
ddl migrate --undo   # restore previous layout
```

## `ddl scope`

Show which `.ddl/` is active.

```bash
ddl scope
```

## Global flags

| Flag | Description |
|------|-------------|
| `-v`, `--verbose` | Enable verbose output |
| `-q`, `--quiet` | Suppress output except errors |
| `-y`, `--yes` | Non-interactive mode |
| `--json` | Output as JSON for machine parsing |

## Exit codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | Partial failure (some tools failed) |
| 2 | Unrecoverable error (invalid args, missing platform) |