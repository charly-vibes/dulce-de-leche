#!/usr/bin/env python3
"""Render the publication and research trees into the book artifact.

Purpose: publish docs/public and docs/research as HTML inside the mdBook
output so the deployed site carries the book, the client-facing publication
pages, and the compiled dossier.
Responsibilities: build throwaway mdBook sources from the Markdown trees,
render them into book/public and book/research, and copy the structured
publication export beside its pages.
Rationale: GitHub Pages serves one site per repository; a second upload would
replace the book. Rendering through mdBook keeps the Markdown sources and the
deployed pages identical instead of maintaining a second HTML copy.

Vendored from charly-vibes/kaya scripts/build-publications.py @ v0.4.1 for
DDL-csx.6. Divergences: titles and the tempdir prefix renamed to ddl; the
tree logic is untouched (no per-repo renderer fork).
"""

from __future__ import annotations

import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

PUBLIC = Path("docs/public")
PUBLIC_OUT = Path("book/public")
RESEARCH = Path("docs/research")
RESEARCH_OUT = Path("book/research")
EXPORT = "publication.json"


def title_of(path: Path) -> str:
    for line in path.read_text(encoding="utf-8").splitlines():
        match = re.match(r"^#\s+(.+)$", line)
        if match:
            return match.group(1).strip()
    return path.stem


def render_tree(source: Path, out: Path, title: str) -> int:
    """Render every Markdown file under `source` into HTML under `out`."""
    pages = sorted(path for path in source.rglob("*.md") if path.name != "SUMMARY.md")
    if not pages:
        print(f"build-publications: {source} has no Markdown pages", file=sys.stderr)
        return 1

    with tempfile.TemporaryDirectory(prefix="ddl-book-") as tmp:
        root = Path(tmp)
        src = root / "src"
        summary = ["# Summary", ""]
        for page in pages:
            relative = page.relative_to(source)
            target = src / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(page, target)
            summary.append(f"- [{title_of(page)}]({relative.as_posix()})")
        (src / "SUMMARY.md").write_text("\n".join(summary) + "\n", encoding="utf-8")
        (root / "book.toml").write_text(
            f'[book]\ntitle = "{title}"\nsrc = "src"\n',
            encoding="utf-8",
        )

        shutil.rmtree(out, ignore_errors=True)
        out.parent.mkdir(parents=True, exist_ok=True)
        process = subprocess.run(
            ["mdbook", "build", str(root), "-d", str(out.resolve())],
            check=False,
        )
        if process.returncode != 0:
            print(f"build-publications: mdbook failed for {source}", file=sys.stderr)
            return process.returncode
    print(f"build-publications: rendered {len(pages)} page(s) into {out}")
    return 0


def main() -> int:
    if not PUBLIC.is_dir():
        print(f"build-publications: {PUBLIC} is missing", file=sys.stderr)
        return 1
    status = render_tree(PUBLIC, PUBLIC_OUT, "ddl publications")
    if status != 0:
        return status
    if RESEARCH.is_dir():
        status = render_tree(RESEARCH, RESEARCH_OUT, "ddl research")
        if status != 0:
            return status
    export = PUBLIC / EXPORT
    if export.is_file():
        shutil.copy2(export, PUBLIC_OUT / EXPORT)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())