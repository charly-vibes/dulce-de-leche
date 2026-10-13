#!/usr/bin/env python3
"""Sync GitHub repo metadata (description, homepage, topics) for the
charly-vibes fleet from the table below — the source of truth.

Every repo gets the `charly-vibes` topic (org filter) and the
`dulce-de-leche` topic (ddl-managed filter). The authoritative fleet URL
combines both (topics AND together; a lone topic:dulce-de-leche also matches
an unrelated external repo):
  https://github.com/search?q=topic%3Acharly-vibes+topic%3Adulce-de-leche&type=repositories

Usage:
  python3 scripts/sync-github-meta.py --dry-run   # show planned changes
  python3 scripts/sync-github-meta.py             # apply via gh repo edit
Requires: gh CLI authenticated (GH_TOKEN), python3.
"""

import argparse
import json
import subprocess
import sys

ORG = "charly-vibes"
DOCS = "https://charly-vibes.github.io"

# Topics applied to every repo in the fleet, on top of the per-repo list.
# `charly-vibes` = org membership filter; `dulce-de-leche` = managed by ddl.
FLEET_TOPICS = ["charly-vibes", "dulce-de-leche"]

# repo -> (description, topics, homepage)
# homepage=None means no book / no docs site — leave unset.
REPOS = {
    "dulce-de-leche": (
        "One command to install, configure, and update every charly-vibes dev tool — the ecosystem orchestrator",
        ["charly-vibes", "cli", "devtools", "rust"],
        f"{DOCS}/dulce-de-leche/",
    ),
    "wai": (
        "CLI workflow manager for AI-driven development — captures the why behind decisions (PARA, sessions, design matrix)",
        ["charly-vibes", "ai", "cli", "rust", "workflow-automation"],
        f"{DOCS}/wai/",
    ),
    "dont": (
        "Epistemic discipline tool for LLM workflows — makes assertions machine-checkable",
        ["charly-vibes", "ai", "cli", "epistemic", "rust"],
        f"{DOCS}/dont/",
    ),
    "espectacular": (
        "Behavioral verification layer for the charly-vibes ecosystem — enforces spec-test correspondence",
        ["charly-vibes", "behavioral-verification", "cli", "rust", "testing"],
        f"{DOCS}/espectacular/",
    ),
    "pretender": (
        "Multi-language code quality checker with complexity metrics, duplication, and mutation testing",
        ["charly-vibes", "cli", "code-quality", "complexity-metrics", "rust"],
        f"{DOCS}/pretender/",
    ),
    "testaruda": (
        "Language-agnostic test selection engine — Datalog rules + provenance semiring over a dependency graph",
        ["charly-vibes", "cli", "rust", "testing", "test-selection"],
        f"{DOCS}/testaruda/",
    ),
    "vampiro": (
        "Cross-language composition checking at call, module, effect, law, retry, resource, and trust boundaries",
        ["charly-vibes", "cli", "code-quality", "composition", "rust"],
        f"{DOCS}/vampiro/",
    ),
    "wild": (
        "Machine-checked compatibility and composition of software components, modelled as a category of contracts",
        ["charly-vibes", "cli", "contracts", "formal-methods", "rust"],
        f"{DOCS}/wild/",
    ),
    "bajan": (
        "Spec-driven knowledge-graph pipeline CLI — ingest sources, extract claims, query the graph",
        ["charly-vibes", "cli", "knowledge-graph", "rust"],
        None,  # no book yet
    ),
    "gently": (
        "Rust port of Perl's Graph::Easy — parse, lay out, and render graphs as ASCII art, boxart, HTML, or DOT",
        ["charly-vibes", "cli", "graphviz", "rust", "visualization"],
        None,  # no book yet
    ),
    "specodelic": (
        "Markdown spec format (Intent / Constraints / Model / Properties) with a CLI that lints, compiles, and verifies it",
        ["charly-vibes", "cli", "rust", "specification"],
        f"{DOCS}/specodelic/",
    ),
    "whisper": (
        "Deterministic knowledge workspace management — repo-local .whisper/ + global ~/.whisper/ routing",
        ["charly-vibes", "cli", "knowledge-base", "rust"],
        f"{DOCS}/whisper/",
    ),
    "genesis": (
        "Shared Rust crate of cross-cutting CLI/AIX/self-healing infrastructure for the charly-vibes tool suite",
        ["charly-vibes", "cli", "infrastructure", "rust"],
        f"{DOCS}/genesis/",
    ),
    "incitaciones": (
        "Reusable prompts and skills for LLM CLIs (pi, Claude Code, Amp, Gemini)",
        ["charly-vibes", "llm", "prompts", "skills", "npm"],
        f"{DOCS}/incitaciones/",
    ),
}


def gh(args: list[str]) -> str:
    return subprocess.run(["gh", *args], capture_output=True, text=True, check=True).stdout


def current(repo: str) -> dict:
    out = gh(["api", f"repos/{ORG}/{repo}", "--jq", "."])
    data = json.loads(out)
    topics = data.get("topics") or []
    return {
        "description": data.get("description"),
        "homepage": data.get("homepage") or None,
        "topics": topics,
    }


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--dry-run", action="store_true")
    ap.add_argument("--repo", action="append", help="limit to specific repo(s)")
    args = ap.parse_args()

    targets = args.repo or list(REPOS)
    changes = 0
    for repo in targets:
        if repo not in REPOS:
            print(f"!! {repo}: not in table, skipping", file=sys.stderr)
            continue
        desc, topics, home = REPOS[repo]
        cur = current(repo)
        wanted = {"description": desc, "homepage": home,
                  "topics": sorted(set([*FLEET_TOPICS, *topics]))}

        diffs = []
        if cur["description"] != wanted["description"]:
            diffs.append(f"description: {cur['description']!r} -> {wanted['description']!r}")
        if cur["homepage"] != wanted["homepage"]:
            diffs.append(f"homepage: {cur['homepage']!r} -> {wanted['homepage']!r}")
        if cur["topics"] != wanted["topics"]:
            diffs.append(f"topics: {cur['topics']} -> {wanted['topics']}")

        if not diffs:
            print(f"ok  {repo} (no change)")
            continue
        changes += 1
        print(f"~   {repo}")
        for d in diffs:
            print(f"      {d}")
        if not args.dry_run:
            if wanted["description"]:
                subprocess.run(["gh", "repo", "edit", f"{ORG}/{repo}",
                                "--description", wanted["description"]], check=True)
            if wanted["homepage"]:
                subprocess.run(["gh", "repo", "edit", f"{ORG}/{repo}",
                                "--homepage", wanted["homepage"]], check=True)
            # PUT replaces the full topic list -> idempotent across runs
            subprocess.run(["gh", "api", "--method", "PUT",
                            f"repos/{ORG}/{repo}/topics", "--input", "-"],
                           input=json.dumps({"names": wanted["topics"]}),
                           text=True, check=True)

    print(f"\n{changes} repo(s) {'would change' if args.dry_run else 'updated'}"
          + (f" out of {len(targets)} checked" if len(targets) > 1 else ""))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
