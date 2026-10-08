---
tags: [architecture, phase-1, genesis]
---

ddl architecture: Phase 1 uses symlink farm — .ddl/<tool>/ -> .<tool>/ — no tool patches needed. Phase 2 uses native config proxy via genesis ConfigFile trait. ddl depends on genesis-vibes for envelope, config, status, doctor, scaffold. Commands: init, install, status, doctor, migrate, upgrade, scope, version. NOT a package manager — invokes brew/cargo/scoop. NOT a unified CLI launcher — each tool keeps its identity. Full design doc at docs/design.md
