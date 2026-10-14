# charly-vibes Tool Ecosystem

> What ddl manages, and how each tool fits in.

For a machine-generated intent → tool table, see the [capability matrix](capability-matrix.md) (regenerated from ddl's registry; drift-checked in CI).

Every repo in the fleet carries the `charly-vibes` + `dulce-de-leche` GitHub
topics — filter the whole set with
[topic:charly-vibes topic:dulce-de-leche](https://github.com/search?q=topic%3Acharly-vibes+topic%3Adulce-de-leche&type=repositories).

## Responsibility map

ddl installs, configures, and updates everything below; each tool owns one
slice of the agent-development loop. Dashed borders mark install-only tools —
ddl installs them, but their state lives outside `.ddl/`.

**Every card below is a link** — click a tool to open its documentation
(in-org tools open their mdBook; external ones open their project page).

<svg viewBox="0 0 760 756" width="100%" style="max-width:760px" role="img" aria-label="charly-vibes responsibility map: ddl on top, four color-coded responsibility bands of tools plus a utilities row, genesis-vibes shared foundation at the bottom; each card links to the tool's docs" xmlns="http://www.w3.org/2000/svg">
  <style>
    .n{font:600 12.5px sans-serif;fill:var(--fg,#EAE1DF)}
    .b{font:600 9.5px ui-monospace,Menlo,Consolas,monospace;fill:var(--inline-code-color,#FFC107)}
    .d{font:9px sans-serif;fill:var(--icons,#B392AC)}
    .band{font:600 9px sans-serif;letter-spacing:1.2px;fill:var(--icons,#B392AC)}
    .t{font:600 15px sans-serif;fill:var(--links,#FF007F)}
    .s{font:9.5px sans-serif;fill:var(--icons,#B392AC)}
    .m{font:9.5px ui-monospace,Menlo,Consolas,monospace;fill:var(--fg,#EAE1DF)}
    a{cursor:pointer}
    a:hover rect{stroke-width:2.5}
    a:hover .n{text-decoration:underline}
  </style>

  <!-- ddl orchestrator -->
  <a href="https://charly-vibes.github.io/dulce-de-leche/">
    <rect x="270" y="14" width="220" height="58" rx="10" fill="var(--quote-bg,#2C2A4A)" stroke="var(--links,#FF007F)" stroke-width="2"/>
    <text x="380" y="38" text-anchor="middle" class="t">ddl</text>
    <text x="380" y="55" text-anchor="middle" class="s">dulce-de-leche · orchestrator</text>
  </a>
  <text x="380" y="96" text-anchor="middle" class="s">one command — install · configure · update</text>
  <line x1="380" y1="102" x2="380" y2="118" stroke="var(--icons,#B392AC)" stroke-width="1.5"/>
  <polygon points="380,126 374,116 386,116" fill="var(--icons,#B392AC)"/>

  <!-- band 1: session & knowledge (blue) -->
  <rect x="40" y="132" width="680" height="104" rx="10" fill="var(--quote-bg,#2C2A4A)" fill-opacity="0.55" stroke="#6E96C8"/>
  <text x="54" y="150" class="band" fill="#6E96C8">SESSION &amp; KNOWLEDGE</text>
  <a href="https://charly-vibes.github.io/wai/">
    <rect x="54" y="158" width="209" height="70" rx="8" fill="var(--bg,#1e1c32)" stroke="#6E96C8"/>
    <text x="66" y="178" class="n">wai</text>
    <text x="66" y="195" class="b">wai</text>
    <text x="66" y="211" class="d">session lifecycle · PARA · handoffs</text>
    <text x="66" y="223" class="d">decision matrix for design</text>
  </a>
  <a href="https://charly-vibes.github.io/whisper/">
    <rect x="275" y="158" width="209" height="70" rx="8" fill="var(--bg,#1e1c32)" stroke="#6E96C8"/>
    <text x="287" y="178" class="n">whisper</text>
    <text x="287" y="195" class="b">turu</text>
    <text x="287" y="211" class="d">deterministic knowledge workspace</text>
    <text x="287" y="223" class="d">.whisper/ + ~/.whisper/ routing</text>
  </a>
  <a href="https://github.com/charly-vibes/bajan">
    <rect x="496" y="158" width="209" height="70" rx="8" fill="var(--bg,#1e1c32)" stroke="#6E96C8"/>
    <text x="508" y="178" class="n">bajan</text>
    <text x="508" y="195" class="b">bajan</text>
    <text x="508" y="211" class="d">knowledge-graph pipeline CLI</text>
    <text x="508" y="223" class="d">ingest · extract · query</text>
  </a>

  <!-- band 2: specs & verification (violet) -->
  <rect x="40" y="246" width="680" height="104" rx="10" fill="var(--quote-bg,#2C2A4A)" fill-opacity="0.55" stroke="#9B7BC8"/>
  <text x="54" y="264" class="band" fill="#9B7BC8">SPECS &amp; VERIFICATION</text>
  <a href="https://charly-vibes.github.io/specodelic/">
    <rect x="54" y="272" width="209" height="70" rx="8" fill="var(--bg,#1e1c32)" stroke="#9B7BC8"/>
    <text x="66" y="292" class="n">specodelic</text>
    <text x="66" y="309" class="b">spk</text>
    <text x="66" y="325" class="d">markdown spec format</text>
    <text x="66" y="337" class="d">lint · compile · verify</text>
  </a>
  <a href="https://charly-vibes.github.io/espectacular/">
    <rect x="275" y="272" width="209" height="70" rx="8" fill="var(--bg,#1e1c32)" stroke="#9B7BC8"/>
    <text x="287" y="292" class="n">espectacular</text>
    <text x="287" y="309" class="b">ah</text>
    <text x="287" y="325" class="d">behavioral verification</text>
    <text x="287" y="337" class="d">spec ⇄ test correspondence</text>
  </a>
  <a href="https://github.com/fission-ai/openspec">
    <rect x="496" y="272" width="209" height="70" rx="8" fill="var(--bg,#1e1c32)" stroke="#9B7BC8" stroke-dasharray="4 3"/>
    <text x="508" y="292" class="n">openspec</text>
    <text x="508" y="309" class="b">openspec</text>
    <text x="508" y="325" class="d">spec-driven change proposals</text>
    <text x="508" y="337" class="d">install-only · external</text>
  </a>

  <!-- band 3: code quality & testing (teal) -->
  <rect x="40" y="360" width="680" height="104" rx="10" fill="var(--quote-bg,#2C2A4A)" fill-opacity="0.55" stroke="#55A78F"/>
  <text x="54" y="378" class="band" fill="#55A78F">CODE QUALITY &amp; TESTING</text>
  <a href="https://charly-vibes.github.io/testaruda/">
    <rect x="54" y="386" width="155" height="70" rx="8" fill="var(--bg,#1e1c32)" stroke="#55A78F"/>
    <text x="64" y="406" class="n">testaruda</text>
    <text x="64" y="423" class="b">testaruda</text>
    <text x="64" y="439" class="d">test selection engine</text>
    <text x="64" y="451" class="d">datalog + provenance</text>
  </a>
  <a href="https://charly-vibes.github.io/pretender/">
    <rect x="219" y="386" width="155" height="70" rx="8" fill="var(--bg,#1e1c32)" stroke="#55A78F"/>
    <text x="229" y="406" class="n">pretender</text>
    <text x="229" y="423" class="b">pretender</text>
    <text x="229" y="439" class="d">structural code quality</text>
    <text x="229" y="451" class="d">complexity · duplication</text>
  </a>
  <a href="https://charly-vibes.github.io/vampiro/">
    <rect x="384" y="386" width="155" height="70" rx="8" fill="var(--bg,#1e1c32)" stroke="#55A78F"/>
    <text x="394" y="406" class="n">vampiro</text>
    <text x="394" y="423" class="b">vampiro</text>
    <text x="394" y="439" class="d">composition checking</text>
    <text x="394" y="451" class="d">call · effect · trust</text>
  </a>
  <a href="https://charly-vibes.github.io/wild/">
    <rect x="551" y="386" width="155" height="70" rx="8" fill="var(--bg,#1e1c32)" stroke="#55A78F"/>
    <text x="561" y="406" class="n">wild</text>
    <text x="561" y="423" class="b">wild</text>
    <text x="561" y="439" class="d">contract compatibility</text>
    <text x="561" y="451" class="d">extract · check</text>
  </a>

  <!-- band 4: epistemics & tracking (amber) -->
  <rect x="40" y="474" width="680" height="104" rx="10" fill="var(--quote-bg,#2C2A4A)" fill-opacity="0.55" stroke="#C89355"/>
  <text x="54" y="492" class="band" fill="#C89355">EPISTEMICS &amp; TRACKING</text>
  <a href="https://charly-vibes.github.io/dont/">
    <rect x="54" y="500" width="209" height="70" rx="8" fill="var(--bg,#1e1c32)" stroke="#C89355"/>
    <text x="66" y="520" class="n">dont</text>
    <text x="66" y="537" class="b">dont</text>
    <text x="66" y="553" class="d">grounds agent claims</text>
    <text x="66" y="565" class="d">evidence state machine</text>
  </a>
  <a href="https://github.com/gastownhall/beads">
    <rect x="275" y="500" width="209" height="70" rx="8" fill="var(--bg,#1e1c32)" stroke="#C89355" stroke-dasharray="4 3"/>
    <text x="287" y="520" class="n">beads</text>
    <text x="287" y="537" class="b">bd</text>
    <text x="287" y="553" class="d">issue tracking with deps</text>
    <text x="287" y="565" class="d">install-only · external</text>
  </a>
  <a href="https://charly-vibes.github.io/incitaciones/">
    <rect x="496" y="500" width="209" height="70" rx="8" fill="var(--bg,#1e1c32)" stroke="#C89355" stroke-dasharray="4 3"/>
    <text x="508" y="520" class="n">incitaciones</text>
    <text x="508" y="537" class="b">npm</text>
    <text x="508" y="553" class="d">agent skills &amp; prompts</text>
    <text x="508" y="565" class="d">install-only · non-Rust</text>
  </a>

  <!-- utilities row (neutral) -->
  <rect x="40" y="588" width="680" height="72" rx="10" fill="var(--quote-bg,#2C2A4A)" fill-opacity="0.55" stroke="#9A9A9A"/>
  <text x="54" y="604" class="band" fill="#9A9A9A">UTILITIES</text>
  <a href="https://github.com/charly-vibes/gently">
    <rect x="300" y="608" width="220" height="46" rx="8" fill="var(--bg,#1e1c32)" stroke="#9A9A9A"/>
    <text x="312" y="626" class="n">gently</text>
    <text x="312" y="640" class="b">gently</text>
    <text x="312" y="651" class="d">graph layout · ASCII · DOT render</text>
  </a>

  <!-- arrow into foundation -->
  <line x1="380" y1="662" x2="380" y2="664" stroke="var(--icons,#B392AC)" stroke-width="1.5"/>
  <polygon points="380,670 374,660 386,660" fill="var(--icons,#B392AC)"/>

  <!-- genesis foundation -->
  <a href="https://charly-vibes.github.io/genesis/">
    <rect x="40" y="672" width="680" height="70" rx="10" fill="var(--quote-bg,#2C2A4A)" stroke="var(--table-border-color,#B392AC)"/>
    <text x="54" y="692" class="band">SHARED FOUNDATION — GENESIS-VIBES (RUST LIBRARY)</text>
    <text x="54" y="713" class="m">envelope · suggestions · managed_block · config · guide · status · doctor · scaffold · suite_linter · git · git_hooks</text>
    <text x="54" y="731" class="d">linked by every genesis-consuming tool · ddl talks to tools over a subprocess protocol (--json)</text>
  </a>
</svg>

## Inclusion criteria

ddl manages a tool when all of the following hold:

1. It is an installable CLI — users run it in a terminal (not a desktop app, library, static site, or content repo).
2. It has (or is about to get) a distribution channel in the charly-vibes tap/bucket.
3. It has a config convention ddl can manage (or is explicitly install-only, like bd, openspec, and incitaciones).

## Active Tools

### Rust CLI tools

| Tool | Binary | Crates.io | Genesis consumer? | Purpose |
|------|--------|-----------|-------------------|---------|
| **wai** | `wai` | `wai-cli` | ✓ | Workflow manager for AI development. PARA-based artifact organization, phase tracking, handoffs. The `matrix` subcommand is the standing deliberation surface from *Design in Practice* — problem, criteria, approaches, and the judgment at each intersection. |
| **dont** | `dont`, `dt` | `dont-cli` | ✓ | Epistemic discipline state machine. Claims, evidence, vocabulary tracking. Cozo DB backend. |
| **espectacular** | `ah` | `espectacular` | ✓ | Behavioral verification. Spec-test correspondence enforcement. |
| **pretender** | `pretender` | `pretender` | ✓ | Multi-language structural code quality. Tree-sitter-based complexity, duplication, mutation testing. |
| **testaruda** | `testaruda`, `testaruda-adapter-rust`, `testaruda-adapter-python` | `testaruda` | ✓ | Test selection engine. Ascent Datalog + provenance semiring. SQLite store. |
| **vampiro** | `vampiro` | `vampiro` | ✓ | Cross-language composition checking at call, module, effect, law, retry, resource, and trust boundaries. Crate workspace with per-language tracers. |
| **wild** | `wild` | `wild-vibes` | — | Machine-checked compatibility and composition of software components, modelled as a category of contracts. `wild extract` / `wild check`. |
| **whisper** | `turu` (aliases: `turututu`, `whisper`) | `whisper-vibes` | ✓ | Deterministic knowledge workspace management. Repo-local `.whisper/` + global `~/.whisper/` routing. Install-only under ddl — its state is home-dir global, not repo-local legacy config. |
| **specodelic** | `specodelic`, `spk` | `specodelic` | ✓ | Markdown spec format (Intent / Constraints / Model / Properties) with the CLI that lints, compiles, verifies, and refactors it. Repo-local `.specs/` corpus. |
| **funkalization** | `funkalization` | `funkalization` | — | Visual semantic design workspace. Versioned design semantics with typed design edits, generated obligations, scoped evidence, and loss-reporting adapters toward Specodelic and OpenSpec. Planned repo-local `.funkalization/` workspace. |
| **bajan** | `bajan` | `bajan` | ✓ | Spec-driven knowledge-graph pipeline CLI — ingest sources, extract claims, query the graph. SQLite-backed triple store, provenance as graph structure. |
| **gently** | `gently` | not yet published | — | Rust port of Perl's Graph::Easy: build graphs, lay them out deterministically, render as ASCII art, boxart, HTML, or Graphviz DOT — plus parse Graph::Easy and DOT input. |

### Non-Rust tools

| Tool | Language | In homebrew? | Notes |
|------|----------|-------------|-------|
| **beads** | Go | ✓ (`beads`, homebrew-core + `gastownhall/beads` tap) | Issue tracker with first-class dependency support (`bd` binary). Release assets are named `beads_*` but ship the `bd` binary. No cargo fallback — the `beads` crate on crates.io is an unrelated package. Install-only — manages repo-local `.beads/` itself. |
| **openspec** | TypeScript/npm | homebrew-core (`openspec`) | Spec-driven development workflow for AI coding agents. npm package `@fission-ai/openspec`; GitHub releases at `fission-ai/openspec`. Install-only — manages the repo-local `openspec/` directory itself. |
| **incitaciones** | TypeScript/npm | npm (`incitaciones`) | Prompt/skill collection for CLI LLM tools. Install-only — skills land in `~/.agents/skills/` or `.agents/skills/`; `ddl init` checks for global skills and prompts if missing. |

## Config file locations

Before ddl:

| Tool | Config location | Format | Notes |
|------|----------------|--------|-------|
| wai | `.wai/` | TOML + directory | PARA structure |
| dont | `.dont/` | Cozo DB + TOML | Heavyweight store |
| espectacular | `.espectacular/` | JSON + TOML | Spec traces |
| pretender | `.pretender.toml` | TOML | Single file |
| testaruda | `.testaruda/` | SQLite + TOML | Dependency graph store |
| vampiro | `.vampiro/` | TOML | Composition config (`config.toml`) |
| specodelic | `.specs/` | Markdown corpus | Spec files |
| funkalization | `.funkalization/` | JSON workspace (planned) | Design workspace |

After ddl migration (Phase 1):

| Tool | Config location | Backwards compat |
|------|-----------------|-----------------|
| wai | `.ddl/wai/` → `.wai/` | ✓ symlink |
| dont | `.ddl/dont/` → `.dont/` | ✓ symlink |
| espectacular | `.ddl/ah/` → `.espectacular/` | ✓ symlink |
| pretender | `.ddl/pretender.toml` → `.pretender.toml` | ✓ symlink |
| testaruda | `.ddl/testaruda/` → `.testaruda/` | ✓ symlink |
| vampiro | `.ddl/vampiro/` → `.vampiro/` | ✓ symlink |
| specodelic | `.ddl/specodelic/` → `.specs/` | ✓ symlink |
| funkalization | `.ddl/funkalization/` → `.funkalization/` | ✓ symlink |

`bd`, `openspec`, `incitaciones`, and `turu` are install-only — their state lives outside `.ddl/` (bd manages repo-local `.beads/` itself; openspec manages `openspec/` itself), so there is nothing to migrate.

## Shared infrastructure (genesis-vibes)

Genesis-consuming tools (wai, dont, espectacular, pretender, testaruda,
vampiro, bajan) depend on `genesis-vibes`; wild and gently do not yet. Genesis
provides:

| Module | Used by | Purpose |
|--------|---------|---------|
| `envelope` | All | Structured JSON CLI output |
| `suggestions` | wai, dont | Typo correction (`DidYouMean`) |
| `managed_block` | wai, dont, ah, testaruda, vampiro | `<!-- BLOCK:START -->` injection, with opt-in provenance footer (`<!-- provenance: generator=… version=… source=… sha=… -->` inside the end marker; hash covers only the content, so drift is machine-checkable) |
| `config` | All | `ConfigFile` trait, `ConfigRegistry` |
| `guide` | All | CLI dispatch, error handling, verbosity |
| `status` | All | `StatusContributor` trait, health aggregation |
| `doctor` | All | `DoctorRunner` with auto-fix |
| `scaffold` | All | `init` command standardization |
| `suite_linter` | testaruda | Cross-tool config linting |
| `fixture` | All | Test helpers |
| `git` | rollout in progress | Read-only git plumbing: repo-root walks, changed-file enumeration, tracked/ignored checks, content hashes — one canonical subprocess-only substrate. |
| `git_hooks` | wai, espectacular, pretender | Git-hook mechanics: install/uninstall with ownership markers, `core.hooksPath` resolution, lefthook managed-block injection. Tool-specific gates stay in the tools. |

## Distribution channels

| Channel | Tools | Maintainer |
|---------|-------|------------|
| crates.io | All released Rust tools (wild publishes as `wild-vibes`; gently not yet published) | Individual repos |
| Homebrew | wai, ah, dont, pretender, testaruda, vampiro, turu, dulce-de-leche — dont formula still a placeholder stub; bd and openspec ship in homebrew-core | `homebrew-charly` tap |
| Scoop | wai, ah, dont, pretender, testaruda, vampiro, turu, dulce-de-leche | `scoop-charly` bucket |
| GitHub Releases | All Rust tools; bd (`gastownhall/beads`); openspec (`fission-ai/openspec`) | Individual repos |
| npm | incitaciones; openspec (`@fission-ai/openspec`) | Individual repos |

## Homebrew formulas (current state)

From `homebrew-charly/`:

| Formula | Version | Has real SHA? | Notes |
|---------|---------|---------------|-------|
| `wai.rb` | 2026.8.5 | ✓ | Real release |
| `testaruda.rb` | 0.4.0 | ✓ | Real release (engine + rust/python adapters) |
| `vampiro.rb` | 0.4.0 | ✓ | Real release |
| `turu.rb` | 0.3.0 | ✓ | Real release (auto-updated by whisper's release workflow) |
| `ah.rb` | 0.5.0 | ✓ | Real release (assets under `espectacular` releases) |
| `pretender.rb` | 0.5.0 | ✓ | Real release |
| `dulce-de-leche.rb` | 0.3.0 | ✓ | Real release |
| `dont.rb` | 0.0.0 | ✗ placeholder | Blocked: no GitHub release for dont yet (tag v0.3.0 exists locally) |

Scoop mirrors the same versions for Windows. All formula hashes were computed from downloaded release assets and cross-verified against each release's published `checksums.txt`.

Only **dont** remains blocked on an upstream release. ddl automatically falls back to `cargo install` for tools whose release binaries are not yet published (except `bd`, which has no crates.io package — the `beads` crate is unrelated), so `ddl init` works either way. (Since DDL-ei3, brew/scoop are not part of ddl's install decisions — the tap and bucket below serve manual installs.)
