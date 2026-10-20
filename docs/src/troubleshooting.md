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
- `specodelic` — Markdown spec format + linter (binary: `spk`)
- `bd` — Issue tracking (beads)
- `openspec` — Spec-driven development
- `incitaciones` — Prompts and skills for CLI LLM tools (npm)
- `turu` — Knowledge workspace management (whisper-vibes)
- `funkalization` — Visual semantic design workspace (experimental)
- `wild` — Contract-based compatibility checking (experimental)

(`ddl` itself is not installable via `ddl install` — it manages every tool
except itself.)

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

## An install or upgrade was interrupted

Tool installs and binary upgrades are staged and only atomically swapped in
after they succeed (see the release notes for 0.9.0). An interrupted install
or upgrade leaves the previous working binary in place — re-run the same
command and it resumes:

```bash
ddl install <tool>   # re-run; the failed attempt left no partial state
ddl upgrade          # same — the old binary stays until the new one verifies
```

If `.ddl/manifest.json` looks inconsistent afterwards, `ddl status` reports
what it sees and `ddl init --no-install` rebuilds the manifest from the
tools already on disk.

## `cargo install bd` installs the wrong crate

The `bd` name on crates.io belongs to an unrelated crate ("big data tool
kit"); `beads` itself is also taken. Beads does not publish to crates.io —
**never `cargo install bd`**. Install `bd` through `ddl install bd`, which
pulls the pinned release binary from the gastownhall GitHub releases.

Deeper postmortems of incidents that cost fleet debugging time (CI install
failures, migration symlink bugs) live in the
[Fleet incident records](./fleet-incidents.md) — maintained under
Contributing.
