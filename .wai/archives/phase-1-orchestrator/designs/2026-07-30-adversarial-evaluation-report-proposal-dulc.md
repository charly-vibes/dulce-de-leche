---
tags: [adversarial-evaluation, stakeholder, pre-mortem, go-no-go]
---

# Adversarial Evaluation Report

## Proposal: dulce-de-leche (ddl) — bundle orchestrator for charly-vibes tools

---

## Tier 1: Stakeholder Council

### 1.1 The Skeptical CFO

**CRITIQUE 1: The addressable market is one person.**

The charly-vibes ecosystem has zero paying users, zero evidence of external adoption, and 6 Homebrew formulas of which only 2 have real releases. The "new user onboarding friction" this project claims to solve is a hypothetical problem for a non-existent audience. You are building a metatool for a tool ecosystem that has not demonstrated it needs bundling. The capital (time, attention, maintenance burden) spent on ddl would be better spent on: shipping the 4 placeholder Homebrew formulas, writing actual documentation for the existing tools, or — most radically — finishing one tool well instead of starting another.

**CRITIQUE 2: Negative ROI from maintenance burden alone.**

Each tool in the ecosystem requires independent maintenance: release cadence, version bumps, CI, issue triage, README updates. ddl adds a **seventh** Rust crate to maintain, with its own release pipeline, its own version compatibility matrix, and its own dependency on genesis. When wai releases a breaking config change, ddl must update its compatible-versions matrix, test against the new version, and cut a release. The cost of keeping ddl synchronized with 6 independent tools compounds. The ROI calculation is: `ddl saves 30 seconds of typing per project init` vs `ddl costs 10+ hours of maintenance per quarter`. That math does not work.

**SYNTHESIS:** This could work if ddl is not a separate Rust crate but a **shell script** (or a single `just` recipe) that takes 50 lines, zero maintenance, and zero release pipeline. The CFO would approve a shell script. The CFO would not approve a Rust crate.

---

### 1.2 The Adversarial Product Lead

**CRITIQUE 1: Phase 1 does not solve the problem it claims to solve.**

The entire value proposition hinges on "repo root stays clean." But Phase 1 uses symlinks: `.ddl/wai/ -> ../.wai/`. The original `.wai/`, `.dont/`, `.espectacular/` directories still exist at the root. They are symlinks now, but `ls -la` shows them, `git status` shows them, editors show them. The root is not clean — it's the same clutter with a different implementation detail. The only way to actually clean the root is Phase 2, which requires patching every tool. And Phase 2 is explicitly deferred ("re-evaluate after 3 months"). So the product ships with a promise that the actual value might arrive later. That is not a product; it's a migration plan.

**CRITIQUE 2: The "bundle" framing is wrong for the actual usage pattern.**

Nobody who uses charly-vibes tools uses all of them. The toolset is an experimental collection, not an integrated suite. wai is a workflow manager. dont is an epistemic state machine. fotos-mcp is an IPC bridge for a screenshot app. These are not complementary tools that a single user would install together. The "bundle" creates artificial coupling between tools that have independent use cases. A user who only wants wai now has to explain to ddl that they don't want the other 6 tools. The minimal install profile (`--profile minimal`) is an admission that the default (all tools) is wrong for most users.

**SYNTHESIS:** This could work if ddl is positioned as a **discovery portal** ("here's what charly tools exist, pick what you need") rather than a bundle manager. The interactive `ddl init` with tool selection is the right UX — but the "One command to install, configure, and update every charly-vibes tool" tagline promises the opposite of what most users need.

---

### 1.3 The Hostile Engineer

**CRITIQUE 1: Subprocess orchestration is fragile and non-deterministic.**

`ddl status` calls subprocesses for each tool. `ddl init` calls subprocesses for each tool's init. `ddl install` calls `brew`/`cargo`/`scoop` subprocesses. This means ddl's reliability is bounded by the worst-behaved subprocess. If `dont prime` hangs (it has a Cozo DB initialization that could lock), ddl hangs. If `brew install` fails because of a network issue, ddl reports a partial failure. The error handling surface is enormous: 6 tools × 3 platforms × N subprocesses each. And ddl has no control over any of these subprocesses — it's a thin shell script pretending to be a compiled binary.

**CRITIQUE 2: The genesis dependency creates a circular coupling problem.**

ddl depends on genesis-vibes. All 6 managed Rust tools also depend on genesis-vibes. When genesis releases a breaking change (e.g., the `ConfigFile` trait adds a required method), ALL 6 tools must update AND ddl must update. This creates a synchronized-release dependency graph where a single genesis change blocks the entire ecosystem. The current design document acknowledges this ("version compatibility matrix") but the solution is reactive (blacklist incompatible versions) rather than structural (keep ddl independent of genesis). If ddl's purpose is to orchestrate tools, it should not share a dependency chain with those tools.

**SYNTHESIS:** This could work if ddl communicates with each tool via a **stable, versioned wire protocol** (JSON over stdin/stdout, like testaruda's adapters) rather than linking against genesis. This decouples ddl's release cycle from the tools' release cycles entirely.

---

### Council Consensus

**The single biggest risk all stakeholders agree on:** ddl is a solution in search of a problem. The "problem" (config sprawl, multi-step install) is a cosmetic inconvenience for an ecosystem with negligible adoption. The cost of building and maintaining a Rust CLI to solve it exceeds the value of the solution. The stakeholders disagree on which aspect is most wasteful (CFO: maintenance cost, Product Lead: Phase 1 doesn't deliver, Engineer: subprocess fragility) but all three converge on the same root: the problem is not big enough to warrant the solution.

---

## Tier 2: Anti-Persona Stress Test

### Anti-Persona 1: "The Weekend Explorer"

**Profile:**
- **Name:** Alex
- **Motivation:** Hears about "resonant coding" and "AI-assisted development tools," wants to try wai after reading a blog post
- **Technical capability:** Medium (comfortable with `brew install`, not a Rust developer)
- **Emotional state:** Curious but impatient — has 30 minutes to evaluate
- **Behavioral pattern:** Follows README instructions literally, gives up on first error

**Attack Scenario:**
1. Entry point: `brew tap charly-vibes/charly && brew install dulce-de-leche`
2. Runs `ddl init` — the interactive prompt asks "Which tools would you like to install?" Alex doesn't know what `dont`, `ah`, `pretender`, `testaruda`, `fotos-mcp`, or `fabbro` are. Selects "all" to avoid decision paralysis.
3. `brew install ah` fails because `ah` formula is at version `0.0.0` with placeholder SHA — brew refuses to install. `brew install dont` also fails. `brew install pretender` also fails. `brew install testaruda` also fails (no formula in homebrew).
4. ddl reports partial failure: "3 of 7 tools installed. wai, fotos-mcp, fabbro: ✓. ah, dont, pretender, testaruda: ✗."
5. Alex doesn't know which of the 4 failed tools are important. `wai` is the one they wanted. The error message is unclear about whether wai works without the others.
6. Alex runs `wai tutorial` — it works. But the experience left a bad taste. The "one command" promised 7 tools but delivered 3. The failure mode was opaque.

**Missing Guardrail:** ddl should detect that 4 of its 7 managed tools have placeholder formulas and refuse to offer them as installable. Or better: ddl should not exist as a separate install path — the user should `brew install wai` directly and discover the ecosystem through wai's documentation.

---

### Anti-Persona 2: "The Toolchain Minimalist"

**Profile:**
- **Name:** Sam
- **Motivation:** Uses wai daily for AI development workflow. Has a clean, minimal setup. Hates bloat.
- **Technical capability:** High (Rust developer, understands Cargo.toml dependencies)
- **Emotional state:** Skeptical, protective of their toolchain
- **Behavioral pattern:** Audits every dependency, removes unused tools, values explicitness

**Attack Scenario:**
1. Entry point: Sam's project already has `.wai/` configured exactly how they want it. They run `ddl init --no-install` to see what ddl does.
2. ddl creates `.ddl/wai/ -> ../.wai/` and announces "migrated." But `.wai/` is now a symlink to `.ddl/wai/`. Sam's editor, git, and shell scripts all see the same files — but the path changed.
3. Sam's CI pipeline has a hardcoded reference to `.wai/AGENTS.md`. The symlink resolves correctly, but Sam doesn't know that. They see an unfamiliar `.ddl/` directory in their repo and don't know if CI will break.
4. Sam runs `git status` — `.ddl/` is untracked. `.wai/` shows as deleted (because it's now a symlink to a path that doesn't exist in the new clone). This is confusing.
5. Sam's reaction: "You added a tool that moves my files around without asking me what layout I want. I had a working setup. Now I have a mystery directory and a symlink I didn't ask for."
6. Sam uninstalls ddl, manually restores the original layout, and adds `ddl` to their personal "never install" list.

**Missing Guardrail:** ddl's `migrate` command should not run automatically during `init`. It should be opt-in with a clear preview: "This will create a `.ddl/` directory and symlink your existing configs. Here's exactly what changes. Proceed? [y/N]". And there should be a `--dry-run` flag.

---

## Tier 3: Pre-Mortem

It is one year from now. dulce-de-leche has failed catastrophically.

### 3.1 Five Causes of Death

| # | Cause | Category | Description | Root Signal Visible Today | Prevention Available Now |
|---|-------|----------|-------------|---------------------------|-------------------------|
| 1 | **Never shipped** | Tiger | The project stays in design phase. The Rule of 5 review, the adversarial evaluation, and the design doc accumulate. The repo has great docs and zero code. After 6 months the user realizes they're maintaining documentation for a tool they haven't built, doing meta-work instead of actual work. | The repo has CLAUDE.md, AGENTS.md, design.md, ecosystem-map.md, a wai workspace, and zero lines of Rust. | Set a hard deadline: 2 weeks to ship a working prototype or kill the project. |
| 2 | **Phase 1 killed by Phase 2 dependency** | Tiger | The design defers real value to Phase 2 ("requires genesis patches"). But Phase 2 requires coordinated releases across 6+ tools. The user never coordinates those releases because each tool has its own priorities. ddl remains in permanent Phase 1 — a symlink farm that doesn't actually clean the root directory. The project is abandoned in a half-finished state. | The design doc explicitly says "Phase 2: requires genesis patches. Re-evaluate after 3 months." This is a deferred-decision smell. | Ship Phase 2 first, or don't promise it. If Phase 1 is the final product, accept that it's a shell script, not a Rust CLI. |
| 3 | **Genesis breaking change orphaned ddl** | Elephant | genesis-vibes v0.4 releases with a breaking change to `ConfigFile` trait. All 6 tools update within 2 weeks. ddl doesn't update because it's low priority. After 3 months, ddl cannot compile against the current genesis. To fix it, the user must update ddl, but the motivation is low because ddl isn't critical to their workflow. The repo rots silently. | ddl's design doc says it depends on genesis-vibes, sharing a dependency chain with all managed tools. This is a structural coupling problem. | Decouple ddl from genesis. Use a stable subprocess protocol instead of shared library linking. |
| 4 | **The "brew install" promise failed on day 1** | Tiger | 4 of 7 Homebrew formulas have placeholder versions (0.0.0 with fake SHA256). When a real user runs `ddl init`, half the installs fail. The user blames ddl, not the individual formulas. First-impression failure kills adoption before it starts. | `homebrew-charly/Formula/ah.rb`, `dont.rb`, `pretender.rb`, `fabbro.rb` all have `version "0.0.0"` and `sha256 "00000..."`. | Ship the 4 placeholder formulas first, BEFORE ddl exists. Or make ddl detect placeholder formulas and refuse to offer them. |
| 5 | **Over-engineered shell script** | Paper Tiger | ddl is a Rust CLI that does: `brew install wai`, `brew install ah`, `brew install dont`. The core logic is 7 `Command::new("brew").arg("install")` calls, wrapped in a progress bar and a JSON envelope. The Rust binary is 8MB compiled. A 50-line bash script would do the same thing with zero maintenance burden. The user realizes this after shipping and feels foolish. | The design doc lists 8 commands, genesis integration, manifest management, version compatibility matrices, and cross-tool status aggregation. The actual problem is: "I want to run 7 brew install commands in sequence." | Before writing any Rust, write the shell script version. If the shell script is 50 lines and works, ship that. If it grows beyond 200 lines, then consider Rust. |

### 3.2 Contradictions

**Technical (trade-off):** **Phase 1 safety** ↔ **Phase 1 value**

The safer the migration model, the less value it delivers. Phase 1 (symlink farm) is maximally safe — zero risk, zero tool patches, zero behavior change. But it also delivers zero observable value: the root directory still has the same clutter, tools behave identically, nothing changes for the user. The only way to deliver real value (clean root, unified config) is Phase 2, which requires coordinated patches across every tool. The safer the approach, the less it's worth doing. The more valuable the approach, the riskier and more expensive it is.

**Technical (trade-off):** **Tight genesis coupling** ↔ **Independent release cadence**

The more ddl integrates with genesis (StatusContributor, ConfigFile, DoctorRunner), the more value it can offer (native status aggregation, config discovery, doctor orchestration). But the tighter the coupling, the more ddl's release cycle is locked to every tool's release cycle. A genesis breaking change requires a coordinated release of all 7 crates (6 tools + ddl). Decoupling via subprocess protocol would make ddl independent but lose the deep integration that justifies its existence. You cannot have both tight integration and independent release cadence.

---

## Verdict

**Survivability: FRAGILE**

- 3 Tigers (never shipped, Phase 2 dependency, brew install failure)
- 1 Elephant (genesis coupling rot)
- 1 Paper Tiger (over-engineering)
- 2 unresolved contradictions (Phase 1 safety vs value, genesis coupling vs independence)

**Top 3 Kill Risks:**

1. **The problem is too small.** The entire justifications boils down to "saves 5 brew install commands on first setup." A shell script does that. The audience for this problem is the user themselves, count=1.
2. **Phase 1 doesn't deliver the headline promise.** "Repo root stays clean" is false in Phase 1. The actual value is deferred to Phase 2, which requires coordinated releases across 6+ tools that may never happen.
3. **4 of 7 managed tools don't actually ship on Homebrew.** The first real user will hit `brew install ah` failing with a placeholder SHA and walk away.

**Conditions for Viability** (from each stakeholder synthesis):

- **CFO:** ddl must be a shell script, not a Rust crate. Zero maintenance burden, zero release pipeline.
- **Product Lead:** ddl must be a discovery portal ("here's what's available, pick what you need"), not a bundle that assumes all tools.
- **Engineer:** ddl must communicate with tools via a stable wire protocol, not a shared genesis dependency that couples release cycles.

These three conditions are incompatible with each other. A shell script cannot maintain a stable wire protocol. A discovery portal is a README, not a CLI. If all three syntheses are requirements, the project is over-constrained and should not be built.

---

## The Real Question

**Does dulce-de-leche add value?**

No — not as a Rust CLI. The value it promises (one-command setup, clean root, cross-tool status) is real but the cost of delivery (7th Rust crate, genesis coupling, release coordination, maintenance) exceeds the value for a single-user ecosystem.

**What would add value?** A 50-line shell script in `homebrew-charly/scripts/bootstrap.sh` that does:

```bash
#!/usr/bin/env bash
set -euo pipefail
brew tap charly-vibes/charly
for tool in wai ah dont pretender testaruda fotos-mcp fabbro; do
  echo "Installing $tool..."
  brew install "$tool" 2>/dev/null || echo "⚠ $tool not available yet"
done
echo "---"
echo "Installed: check with: brew list | grep charly"
echo "Configure: cd my-project && wai init && dont prime && ah init"
```

That script costs zero to maintain, ships today, and solves 80% of the stated problem. The remaining 20% (cross-tool status, `.ddl/` convention) is not worth a Rust binary.
