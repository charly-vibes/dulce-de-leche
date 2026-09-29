# Tasks: Update Spec Tool Tables

## 1. Spec deltas

- [x] 1.1 `bootstrap` delta: Tool Name Mapping prose replaced with the current 11-tool registry table (fotos-mcp/fabbro removed; testaruda formula real; vampiro/turu/specodelic added)
- [x] 1.2 `bootstrap` delta: Placeholder Detection prose — name list reduced to the tools whose formulas are actually placeholders today (`dont`)
- [x] 1.3 `health` delta: Discovery mode scenario scans the current 11 binaries
- [x] 1.4 `dot-ddl` delta: Standard layout scenario lists the actual legacy-config-backed `.ddl/` entries (no fabbro)
- [x] 1.5 `project.md` deltas: purpose sentence + Managed Tools table updated to the 11-tool registry

## 2. Validation

- [x] 2.1 `openspec validate update-spec-tool-tables --strict` passes
- [x] 2.2 `rg 'fotos-mcp|fabbro' openspec/` returns no stale managed-tool references (historical/archive mentions excluded)
