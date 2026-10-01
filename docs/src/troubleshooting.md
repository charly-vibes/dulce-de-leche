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
- `bd` — Issue tracking (beads)
- `openspec` — Spec-driven development
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
## Fleet incidents: CI installs and migration

Symptom → cause → fix records from incidents that actually cost fleet
debugging time. If CI or migration misbehaves, check here first.

### Beads release tarball clobbers repo checkouts

| | |
| :--- | :--- |
| **Symptom** | Conformance tests report README/LICENSE/CHANGELOG drift in repos that were not touched — across several repos at once, right after adding a beads install step to CI |
| **Cause** | `tar -xzf bd.tgz` run in the repo root: the beads release archive ships its own `README`, `LICENSE`, `CHANGELOG`, which overwrote the checkout's files. Root-caused with a temporary `eprintln` probe pushed to CI |
| **Fix** | Extract into a temp dir and copy only the `bd` binary member |

```yaml
- run: |
    dir=$(mktemp -d)
    curl -fsSL <release-asset-url> | tar -xzf - -C "$dir" bd
    install -m 0755 "$dir/bd" /usr/local/bin/bd
```

### CI jobs cancelled with no failure — cold cargo install batches

| | |
| :--- | :--- |
| **Symptom** | CI runs "cancelled" sporadically, no test failure, no log error — first seen in dont |
| **Cause** | A batch of pinned `cargo install`s on a cold cache exceeds the default 10-minute job timeout; GitHub marks the run cancelled, which reads like a flake |
| **Fix** | Bump the install step to 30 minutes fleet-wide (§4 dogfood matrix); keep installs pinned so cache-cold batches are predictable |

### `cargo install bd` installs the wrong crate

| | |
| :--- | :--- |
| **Symptom** | Install of beads "succeeds" but the `bd` binary is not beads |
| **Cause** | The `bd` name on crates.io belongs to an unrelated crate ("big data tool kit"); `beads` itself is also taken. Beads does not publish to crates.io |
| **Fix** | Install from the gastownhall GitHub release asset only, pinned to a version (§4 dogfood matrix). Never `cargo install bd` |

### Migration symlink incidents

| | |
| :--- | :--- |
| **Symptom** | `ddl migrate --undo` silently finds nothing to undo; re-running `ddl migrate` fails with `Not a directory`; single-file configs (e.g. `.pretender.toml`) became *directory* symlinks |
| **Cause** | Symlink targets persisted as relative paths while later discovery compared absolute paths (unnormalized), so detection saw nothing; `migrate_tool()` had no already-migrated guard; single-file configs were linked as directories |
| **Fix** | Normalize symlink targets before comparing (`migrated_tools()`); guard with `is_symlink(legacy_path)` and skip; separate single-file link creation/restoration (shipped as DDL-0yp, DDL-el9, DDL-cbp, DDL-40e) |
