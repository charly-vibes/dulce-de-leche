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

ddl init --gates
# Wire the end state users actually ask for: lefthook hard gates
# (pre-commit: ah check, pretender gate, spk lint; pre-push: ah check),
# .gitignore entries for tool data dirs (.testaruda/, .pretender/), and
# beads no-db stamping. Also orders tool inits so openspec runs before ah.
# Idempotent — safe to re-run. Follow with `lefthook install`.

ddl init --yes --gates
# Full bootstrap with gates wiring — the one-shot setup
```

### Init cascade semantics

Tool inits run in a prerequisite order (openspec before ah), with the
`.ddl/bin` directory on the PATH so freshly downloaded binaries can be
initialized, and tools without an init command are skipped silently.

- A failing tool init **fails the run** with a resume summary listing the
  exact command to finish manually (e.g. `dont init`) or the selective
  re-run (`ddl init --tools dont`). No more exit-0 half-configured repos.
- Successful inits are recorded in `.ddl/manifest.json`; re-runs skip them
  (idempotent-until-green) instead of failing on tools that refuse
  re-initialization.
- Non-interactive mode (`--yes`/`--json`) passes prompt-free flags to tools
  with wizards (e.g. `pretender init --non-interactive`).

## `ddl install <tool>`

Install a single tool by name.

```bash
ddl install wai
ddl install dont
ddl install ah
ddl install pretender
ddl install testaruda
ddl install vampiro
ddl install bd
ddl install openspec
ddl install incitaciones
ddl install turu
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

## `ddl feedback <kind>`

File a feedback issue against ddl's own repository
(`charly-vibes/dulce-de-leche`) via `gh`. The repository URL is compiled
into the binary, so this works from any directory — no repo spelunking.

> Note: the binary is `ddl`, but the crate is `dulce-de-leche`
> (`cargo install dulce-de-leche`). Looking for `charly-vibes/ddl` on
> GitHub returns 404 — the canonical repository is
> `charly-vibes/dulce-de-leche`.

```bash
ddl feedback bug                  # kind: bug|feature|question|chore
ddl feedback bug --dry-run        # print issue body + gh command, don't submit
ddl feedback bug --from-last-error  # prefill body from the last failed ddl command
echo "what happened" | ddl feedback bug  # pipe body content via stdin
ddl feedback bug --title "custom title"  # override the derived title
```

The issue body is pre-filled with environment context automatically: ddl
version, OS/arch, shell, `gh` version, current git remote/branch/dirty
state, repo tooling state, and a repro hash. Without network access, the
body is saved to a local file so nothing is lost.

## Global flags

| Flag | Description |
|------|-------------|
| `-v`, `-vv`, `-vvv` | Enable verbose output (progressive detail) |
| `-q`, `--quiet` | Suppress output except errors |
| `-y`, `--yes` | Non-interactive mode |
| `--json` | Output as JSON for machine parsing |
| `--human` | Force human-readable output in non-TTY contexts |
| `--verbose` | Alias for `-v` |

## Exit codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | Error (any failure — see stderr for details) |