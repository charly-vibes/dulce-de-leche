# Health Delta

## MODIFIED Requirements

### Requirement: Status Command

The CLI SHALL provide a `ddl status` command that shows a health overview of the managed charly-vibes tools.

#### Scenario: Discovery mode (no manifest)

- **WHEN** user runs `ddl status` and `.ddl/manifest.json` does not exist
- **THEN** the system scans PATH for known tool binaries (wai, dont, ah, pretender, testaruda, vampiro, turu, specodelic, bd, openspec, incitaciones)
- **AND** reports each found tool as "detected (PATH)"
- **AND** suggests `ddl init` to create a manifest
- **AND** exits with code 0 if all tools found, code 1 if some missing

#### Scenario: Installed tools summary

- **WHEN** user runs `ddl status` with an existing `.ddl/manifest.json`
- **THEN** the system reports the installed version and status of each recorded tool
- **AND** reports each tool's adoption category and maturity (core/stable, recommended/working, extension/spec-stage, …)
