# Setup Wizard

`ddl setup` is the guided first-time setup: one command that walks from zero
to a verified-initialized repo. Where `ddl init` is the raw bootstrap
cascade, `ddl setup` adds a pre-flight check, asks fresh-vs-migrate and a
scope question, then runs the **same** install cascade `ddl init` uses — the
wizard orchestrates, it never reimplements.

```bash
ddl setup
```

## The five steps

The wizard runs five ordered steps (openspec `add-setup-wizard-onboarding`
§2.2). Each step runs exactly once, in this order:

| Step | What happens |
|------|--------------|
| **pre-flight** | Platform + prerequisite checks (the same checks `ddl init` runs), surfaced as a list with a human-readable fix for each failure. |
| **migrate** | Fresh `.ddl/` config vs migrating existing configs — the migrate dry-run report is shown before anything is applied. |
| **scope** | Full fleet vs a subset — a category (core / recommended / extension) or an explicit tool list. The registry categories are the taxonomy. |
| **delegate** | The install + tool-init cascade, via the exact code paths `ddl init` uses (`installer::install_tool` + `installer::run_tool_init`). |
| **verify** | Ends with the `ddl status` / `ddl doctor` verification pass. |

## Flags

```bash
ddl setup --tools wai,dont   # Scope to specific tools only
ddl setup --no-install       # Configure only — skip installation
ddl setup --yes              # Non-interactive: no prompts (implied by --json)
ddl setup --json             # Machine-readable envelope
ddl setup --verbose          # More detail per step
```

The wizard-level flags forward into the step machine; `-y`/`--json` also
pass prompt-free flags to tools with wizards (e.g. `pretender init
--non-interactive`), the same non-interactive rule as `ddl init`.

## Failure and resume

A failing step does **not** exit 0. Failures fold into a resume summary —
the exact commands to finish manually or the selective re-run (e.g.
`ddl setup --tools dont`) — and the exit code is non-zero. This is the same
contract as `ddl init`: no half-configured repos that claim success. Re-runs
skip tools already recorded in `.ddl/manifest.json` (idempotent-until-green).

## JSON envelope

With `--json`, stdout carries exactly one envelope with `data.command ==
"setup"`. The machine verdict lives in `data.ok` (the envelope-level `ok`
stays true: the command ran and reported; failures travel via the exit code
and stderr):

```json
{
  "data": {
    "command": "setup",
    "tools": ["wai"],
    "no_install": false,
    "yes": true,
    "steps": [
      { "step": "pre-flight", "status": "ok", "failures": [], "detail": null }
    ],
    "ok": true,
    "resume_summary": []
  }
}
```

## When to use what

- **New machine, first time in a charly-vibes repo:** `ddl setup` — it
  checks prerequisites before touching anything and verifies at the end.
- **You know exactly what you want:** `ddl init --yes --gates` (or
  `ddl setup --yes`) — same cascade, no questions.
- **One tool:** `ddl install <tool>`.
- **Existing legacy configs:** `ddl migrate` first, or let the wizard's
  migrate step do it after showing you the dry-run report.
