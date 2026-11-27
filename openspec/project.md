# Project

## Purpose

**dulce-de-leche (ddl)** will enable **agents and human maintainers of charly-vibes repositories** to **install, configure, diagnose, and upgrade the full 17-tool charly-vibes ecosystem with zero manual per-tool steps** by **collapsing install, config migration, health diagnosis, and version upgrades into the single commands `ddl init`, `ddl install`, `ddl migrate`, `ddl status`, `ddl doctor`, `ddl upgrade`, and `ddl scope`** within **one `ddl init` run under 30 minutes for a clean machine and one `ddl upgrade` run under 5 minutes for the whole fleet**, as measured by **`ddl doctor` reporting 17/17 managed tools healthy with zero drift findings**, compared to **the pre-ddl baseline of a dozen independent install paths, per-tool config formats, and untracked version drift across the ecosystem**.

**Kill criteria:** if by **2027-03-01** `ddl doctor` cannot reach ≥15/17 managed tools healthy on clean macOS, Linux, and Windows machines in under 30 minutes from bare `ddl init`, or a routine `ddl upgrade` still requires more than 15 minutes of documented manual intervention per release cycle, the ecosystem-orchestrator value proposition is falsified and **charly** (project owner) must either re-scope ddl to the subset that works or archive the tool.
