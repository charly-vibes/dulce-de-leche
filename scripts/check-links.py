#!/usr/bin/env python3
"""Check every relative link in the built book.

Purpose: catch a deployed page that points outside the Pages artifact before
it ships (the publication link used to escape `book/`).
Responsibilities: walk `book/**/*.html`, resolve every relative href/src
against its page, and fail naming each unresolved target.
Rationale: the check is internal-only — no network access, no HTML parser
dependency — and it judges exactly the bytes the Docs workflow uploads.

Vendored verbatim from charly-vibes/kaya scripts/check-links.py @ v0.4.1
(DDL-csx.6).
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path
from urllib.parse import unquote, urlsplit

LINK = re.compile(r'(?:href|src)="([^"]*)"')


def is_external(target: str) -> bool:
    if not target or target.startswith("#"):
        return True
    if target.startswith("//"):
        return True
    parts = urlsplit(target)
    return bool(parts.scheme)


def resolve(page: Path, target: str, root: Path) -> Path | None:
    """The filesystem target a relative link must reach, or None if absent."""
    parts = urlsplit(target)
    relative = unquote(parts.path)
    if not relative:
        return None
    if relative.startswith("/"):
        candidate = (root / relative.lstrip("/")).resolve()
    else:
        candidate = (page.parent / relative).resolve()
    if not candidate.is_relative_to(root):
        return None
    if candidate.is_file():
        return candidate
    if candidate.is_dir() and (candidate / "index.html").is_file():
        return candidate / "index.html"
    return candidate if candidate.is_file() else None


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--book", default="book", help="built book directory")
    args = parser.parse_args()
    root = Path(args.book).resolve()
    if not root.is_dir():
        print(f"check-links: {args.book} is missing; build the book first", file=sys.stderr)
        return 2

    failures = 0
    pages = 0
    for page in sorted(root.rglob("*.html")):
        pages += 1
        text = page.read_text(encoding="utf-8", errors="replace")
        for target in LINK.findall(text):
            if is_external(target):
                continue
            if resolve(page, target, root) is None:
                failures += 1
                print(
                    f"unresolved link: {page.relative_to(root)} -> {target}",
                    file=sys.stderr,
                )

    if failures:
        print(f"check-links: {failures} unresolved link(s) in {pages} page(s)", file=sys.stderr)
        return 1
    print(f"check-links: {pages} page(s), 0 unresolved links")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())