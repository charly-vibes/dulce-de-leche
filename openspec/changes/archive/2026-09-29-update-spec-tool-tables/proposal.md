# Update Spec Tool Tables: Align Docs With the Current Tool Registry

## Why

`openspec/specs/bootstrap/spec.md`, `health/spec.md`, and `dot-ddl/spec.md` still list **fotos-mcp** and **fabbro** in their tool tables (bootstrap:30-31, health:93, dot-ddl:59), and `openspec/project.md` names both in its purpose line and Managed Tools table. Both tools were removed from ddl's registry long ago (fotos-mcp archived; fabbro frozen), and the tables were already stale before that cleanup — they also predate the registry additions of vampiro, turu, incitaciones, bd, openspec, and specodelic (commit 1228d77 set the precedent of leaving specs untouched while `MANAGED_TOOLS` evolved). The generated `docs/capability-matrix.md` and the drift tests now keep README/ecosystem-map honest; the openspec specs are the last stale surface.

This also feeds **DDL-gap**: remove fabbro and fotos-mcp references from the openspec specs tool tables.

## What Changes

- `openspec/specs/bootstrap/spec.md` (Design Rationale prose):
  - Tool Name Mapping table replaced with the current 11-tool registry mapping (name, crate/npm, formula, repo) — fotos-mcp and fabbro removed; testaruda now has a real formula; vampiro/turu/specodelic rows corrected
  - Placeholder Detection list updated: only `dont` (and the retired fabbro, now gone) has a placeholder formula; ah/pretender/testaruda/turu/vampiro formulas are real
- `openspec/specs/health/spec.md`: the `ddl status` discovery-mode scenario scans the current 11 binaries instead of listing fotos-mcp/fabbro
- `openspec/specs/dot-ddl/spec.md`: the standard `.ddl/` layout drops `.ddl/fabbro/` and matches the actual legacy-config mapping (wai, dont, ah, pretender.toml, testaruda, vampiro, specodelic)
- `openspec/project.md`: purpose sentence and Managed Tools table updated to the same 11 tools (fotos-mcp/fabbro rows removed, vampiro marked Active, turu/incitaciones/bd/openspec/specodelic added)

**Explicitly out of scope (unchanged):** the Placeholder Detection requirement's *mechanics* (MUST detect placeholder formulas) — its semantic status under the DDL-ei3 install policy (brew/scoop no longer in ddl's install decisions) deserves its own spec pass; here we only stop citing retired tools. No code changes; no behavioral change to ddl.

## Impact

- Affected specs: `bootstrap` (prose: Tool Name Mapping, Placeholder Detection), `health` (Status Command — Discovery mode scenario), `dot-ddl` (Directory Layout — Standard layout scenario), `project.md` (purpose + Managed Tools table)
- Affected code: none — documentation-only change
- Risk: zero behavioral risk; purely removes stale references
