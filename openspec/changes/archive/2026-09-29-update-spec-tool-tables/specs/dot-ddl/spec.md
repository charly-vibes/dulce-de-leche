# Dot-DDL Delta

## MODIFIED Requirements

### Requirement: Directory Layout

The `.ddl/` directory SHALL follow a standard layout.

#### Scenario: Standard layout after init

- **WHEN** user runs `ddl init` or `ddl init --yes`
- **THEN** the system creates `.ddl/` with the following structure:
  - `.ddl/manifest.json` — version manifest
  - `.ddl/config.toml` — ddl's own config
  - `.ddl/wai/` — wai config directory
  - `.ddl/dont/` — dont config directory
  - `.ddl/ah/` — espectacular config directory
  - `.ddl/pretender.toml` — pretender config file
  - `.ddl/testaruda/` — testaruda config directory
  - `.ddl/vampiro/` — vampiro config directory
  - `.ddl/specodelic/` — specodelic config directory

#### Scenario: Per-tool subdirectory

- **WHEN** a tool is installed via `ddl install <tool>`
- **THEN** the system creates the corresponding subdirectory under `.ddl/<tool>/`
- **AND** the subdirectory is a symlink to the tool's legacy config directory in Phase 1
- **AND** install-only tools (bd, openspec, incitaciones, turu) have no `.ddl/` subdirectory — they manage their own repo-local or home-dir state
