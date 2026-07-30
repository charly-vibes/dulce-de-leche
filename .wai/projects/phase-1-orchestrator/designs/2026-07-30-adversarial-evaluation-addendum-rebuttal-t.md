---
tags: [addendum, rebuttal, multi-platform, revised-scope]
---

# Adversarial Evaluation — Addendum

## Rebuttal to the "shell script" conclusion

The original evaluation concluded: "a 50-line shell script would solve 80% of the problem." That conclusion had a hidden assumption: **Homebrew is available everywhere.** It's not.

| Environment | Has brew? | Has cargo? | Has scoop? | Has a shell? | Needs a binary |
|-------------|-----------|------------|------------|--------------|----------------|
| macOS laptop | ✓ | ✓ | ✗ | ✓ | ✗ (brew works) |
| Ubuntu CI runner | ✗ | maybe | ✗ | ✓ | ✓ |
| Docker (scratch/alpine) | ✗ | ✗ | ✗ | maybe | ✓ (static binary) |
| Windows CI | ✗ | ✗ | maybe | ✓ | ✓ |
| NixOS | ✗ | ✗ | ✗ | ✓ | ✓ |
| Server (RHEL) | ✗ | maybe | ✗ | ✓ | ✓ |
| Air-gapped network | ✗ | ✗ | ✗ | ✓ | ✓ (vendored) |

A shell script that calls `brew install` **only works on macOS**. Cargo requires the Rust toolchain to be installed first (chicken-and-egg for bootstrap). The only universal bootstrap mechanism is **a pre-compiled static binary that works everywhere**.

This is the actual value of ddl: **one binary, one command, any platform, no prerequisites.**

---

## Revised assessment

### What the original evaluation got wrong

1. **Assumed the user is "someone else."** The evaluation kept saying "the audience for this is count=1." But that count=1 is *you* — working across multiple environments. You use these tools on your laptop, in CI, maybe in containers. The cross-platform bootstrap problem is real for you.

2. **Dismissed cross-platform as a non-problem.** "Just use brew" doesn't work on Linux CI runners. "Just use cargo" requires the Rust toolchain. "Just use the GitHub release" requires knowing which URL to download. ddl automates that discovery.

3. **Missed the CI reproducibility angle.** A `ddl.lock` or `.ddl/manifest.json` that pins tool versions, combined with a single `ddl init --yes` command, means CI always gets the same tools. No "works on my machine" for the toolchain itself.

### What the original evaluation got right

1. **Phase 1 symlink farm doesn't deliver the headline promise.** "Clean root" is Phase 2. This is still true. If the value proposition is cross-platform bootstrap, the `.ddl/` directory is secondary and should be positioned as such.

2. **4 of 7 Homebrew formulas are placeholder.** `ah`, `dont`, `pretender`, `testaruda` have version `0.0.0` with fake SHA256. If ddl tries to `brew install` them, it fails. ddl must detect this and offer a fallback (cargo install or binary download).

3. **Genesis coupling is a real maintenance risk.** Still true. The countermeasure is to keep ddl's interaction surface with each tool minimal: call subprocesses, don't link against genesis. `ddl status` can parse `wai status --json` output rather than importing genesis's `StatusContributor` trait.

---

## Updated proposal

Shift the value proposition from **"clean root directory"** to **"cross-platform bootstrap + version pinning."**

```
ddl — one binary, one command, any platform.

  ddl init          → install & configure all charly tools everywhere
  ddl init --yes    → non-interactive, for CI
  ddl version       → show versions of all tools
  ddl upgrade       → update everything
  ddl status        → check health of all tools
```

The `.ddl/` directory is a nice-to-have (the "why not" organization), not the headline. The headline is: **download one binary, run one command, get the whole toolset working on any platform.**

This reframing:
- Makes the CFO happy (a Rust binary for cross-platform distribution is justified, unlike a shell script that only works on macOS)
- Makes the Product Lead happy (clear value proposition: "works everywhere")
- Makes the Engineer happy (subprocess protocol, not genesis linking)
- Addresses your question: "places where brew is not available"

---

## What changes

### Keep
- The `ddl` binary, cross-compiled for macos/linux/windows, amd64/arm64
- `ddl init` as the single entry point
- `ddl status` / `ddl upgrade` / `ddl version`
- Platform detection + appropriate installer
- `.ddl/manifest.json` for version pinning
- `.ddl/` as the config directory (but not the headline)

### Drop (or defer)
- Phase 2 native config proxy — not needed if `.ddl/` is just organization
- `ddl migrate` — if the value is bootstrap, not re-organization
- `ddl scope` — only needed for monorepo workspaces, which is a niche case
- genesis dependency — use subprocess calls instead of shared library linking
- The "clean root" promise — it's not the real value

### Prioritize
- **Binary releases for all platforms** (macOS arm64, macOS amd64, Linux arm64, Linux amd64, Windows amd64)
- **`ddl init --yes` for CI** — non-interactive, reads from `.ddl/manifest.json` or a lockfile
- **Fallback chain**: binary download → cargo install → brew/scoop install
- **Detect placeholder formulas** and skip them with a clear message

This is a smaller, more focused project that delivers real value for the actual user (you, across environments). The Rule of 5 verdict moves from NEEDS_REVISION to READY — as long as the scope is adjusted to match the real problem.
