---
tags: [ecosystem, research]
---

Ecosystem survey of charly-vibes tools — 6 active Rust CLI tools (wai, dont, ah, pretender, testaruda, fotos-mcp), 1 Go tool (fabbro), 1 in spec (vampiro). All Rust tools depend on genesis-vibes v0.3 for shared infrastructure. Current distribution: 7 Homebrew formulas (only wai and fotos-mcp have real releases), 7 Scoop packages, all on crates.io. Config sprawl: each tool has its own directory/file at repo root. This is the primary pain point ddl solves.
