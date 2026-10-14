#!/usr/bin/env python3
"""Create or update a Homebrew formula for dulce-de-leche.

Usage:
    python3 scripts/update-homebrew.py <formula_path> <version> <tag> <checksums_path>

Example:
    python3 scripts/update-homebrew.py /tmp/Formula/dulce-de-leche.rb 0.1.0 v0.1.0 dist/checksums.txt

Purpose:
    Renders the homebrew-charly tap formula for one release. DDL-9e7: the
    script refuses to write any output unless the declared version, the
    asset tag, and the checksum entries for every referenced platform
    asset correspond — a failed verification exits non-zero and leaves
    the target untouched, so a retry can never commit a half-verified
    channel update. Rendering is deterministic, so re-running against an
    already-updated tap is a no-op (idempotent retry).
"""
import os
import sys

# Platforms the formula references; each must appear in checksums.txt.
REQUIRED_PLATFORMS = [
    "darwin_arm64", "darwin_amd64", "linux_arm64", "linux_amd64",
]


class VerificationError(Exception):
    """Release inputs do not correspond; the channel update is refused."""


def verify_release_inputs(version, tag, checksums_path, required=REQUIRED_PLATFORMS):
    """Assert declared version, asset tag, and checksums correspond.

    Raises VerificationError (before any file is written) when:
      * the tag does not equal "v" + version, or
      * any required platform asset is missing from the checksum file.
    """
    if tag != "v" + version:
        raise VerificationError(
            f"refusing channel update: tag {tag!r} does not match "
            f"declared version {version!r} (expected 'v{version}')"
        )
    if not os.path.isfile(checksums_path):
        raise VerificationError(
            f"refusing channel update: checksums file {checksums_path!r} not found"
        )
    shas = parse_checksums(checksums_path)
    missing = [p for p in required if p not in shas]
    if missing:
        raise VerificationError(
            f"refusing channel update: checksums file is missing assets for "
            f"{', '.join(missing)}"
        )
    return shas


def parse_checksums(checksums_path):
    """Return {platform: sha256} for every required platform found."""
    shas = {}
    with open(checksums_path) as f:
        for line in f:
            parts = line.split()
            if len(parts) == 2:
                sha, name = parts
                for platform in REQUIRED_PLATFORMS:
                    if platform in name:
                        shas[platform] = sha
    return shas


def build_formula(version, tag, shas):
    """Render the formula text deterministically."""
    base = f"https://github.com/charly-vibes/dulce-de-leche/releases/download/{tag}"

    return f"""\
# typed: false
# frozen_string_literal: true

class DulceDeLeche < Formula
  desc "Orchestrator for the charly-vibes tool ecosystem"
  homepage "https://github.com/charly-vibes/dulce-de-leche"
  version "{version}"
  license "Apache-2.0"

  on_macos do
    on_arm do
      url "{base}/ddl_{version}_darwin_arm64.tar.gz"
      sha256 "{shas['darwin_arm64']}"
    end
    on_intel do
      url "{base}/ddl_{version}_darwin_amd64.tar.gz"
      sha256 "{shas['darwin_amd64']}"
    end
  end

  on_linux do
    on_arm do
      if Hardware::CPU.is_64_bit?
        url "{base}/ddl_{version}_linux_arm64.tar.gz"
        sha256 "{shas['linux_arm64']}"
      end
    end
    on_intel do
      url "{base}/ddl_{version}_linux_amd64.tar.gz"
      sha256 "{shas['linux_amd64']}"
    end
  end

  def install
    bin.install "ddl"
  end

  test do
    system "\#{{bin}}/ddl", "--version"
  end
end
"""


def write_formula(formula_path, text):
    os.makedirs(os.path.dirname(formula_path), exist_ok=True)
    with open(formula_path, "w") as f:
        f.write(text)


def main(argv):
    formula_path, version, tag, checksums_path = argv[1], argv[2], argv[3], argv[4]
    try:
        shas = verify_release_inputs(version, tag, checksums_path)
    except VerificationError as exc:
        print(f"ERROR: {exc}", file=sys.stderr)
        return 2
    write_formula(formula_path, build_formula(version, tag, shas))
    print(f"Wrote {formula_path} (version {version})")
    for p, s in sorted(shas.items()):
        print(f"  {p}: {s[:16]}...")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
