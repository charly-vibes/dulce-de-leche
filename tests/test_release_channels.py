#!/usr/bin/env python3
"""Regression fixtures for DDL-9e7 (SEC-07): release channel propagation.

Purpose:
    Prove that Homebrew/Scoop channel propagation failures surface as a
    visible incomplete-distribution result, that retry is idempotent, and
    that declared version, asset tag, and checksums are verified before a
    channel update is committed.

Responsibilities:
    * Functional tests against scripts/update-homebrew.py and
      scripts/update-scoop.py (imported as modules, exercised via main()).
    * Structural tests against .github/workflows/release.yml proving the
      channel steps distinguish disabled channels from real failures,
      retry idempotently, and record release-health results.

Rationale:
    Both distribution update steps previously used continue-on-error,
    allowing a release to appear successful while a channel lagged behind
    (SEC-07 evidence). These fixtures are the required meter:
    python3 -m unittest discover -s tests -p "test_release_channels*.py"
"""

import importlib.util
import json
import os
import subprocess
import sys
import tempfile
import unittest

import yaml

REPO = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
RELEASE_YML = os.path.join(REPO, ".github", "workflows", "release.yml")


def load_script(name):
    path = os.path.join(REPO, "scripts", name)
    spec = importlib.util.spec_from_file_location(name.replace("-", "_").replace(".py", ""), path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


CHECKSUMS = """\
{sha_arm64}  ddl_0.2.0_darwin_arm64.tar.gz
{sha_amd64}  ddl_0.2.0_darwin_amd64.tar.gz
{sha_l64}  ddl_0.2.0_linux_arm64.tar.gz
{sha_lamd64}  ddl_0.2.0_linux_amd64.tar.gz
{sha_win}  ddl_0.2.0_windows_amd64.zip
""".format(
    sha_arm64="a" * 64,
    sha_amd64="b" * 64,
    sha_l64="c" * 64,
    sha_lamd64="d" * 64,
    sha_win="e" * 64,
)


class ScriptVerificationTest(unittest.TestCase):
    """AC: version, tag, and checksum must correspond before committing."""

    def setUp(self):
        self.tmp = tempfile.mkdtemp()
        self.checksums = os.path.join(self.tmp, "checksums.txt")
        with open(self.checksums, "w") as f:
            f.write(CHECKSUMS)

    def test_homebrew_rejects_mismatched_tag(self):
        """tag must equal 'v' + declared version or the update is refused."""
        mod = load_script("update-homebrew.py")
        with self.assertRaises(mod.VerificationError):
            mod.verify_release_inputs("0.2.0", "v0.1.9", self.checksums)

    def test_scoop_rejects_mismatched_tag(self):
        mod = load_script("update-scoop.py")
        with self.assertRaises(mod.VerificationError):
            mod.verify_release_inputs("0.2.0", "v0.1.9", self.checksums)

    def test_homebrew_rejects_missing_platform_checksum(self):
        """Every platform asset the formula references must be in checksums."""
        mod = load_script("update-homebrew.py")
        with open(self.checksums, "w") as f:
            f.write("d" * 64 + "  ddl_0.2.0_linux_amd64.tar.gz\n")
        with self.assertRaises(mod.VerificationError):
            mod.verify_release_inputs("0.2.0", "v0.2.0", self.checksums)

    def test_scoop_rejects_missing_windows_checksum(self):
        mod = load_script("update-scoop.py")
        with open(self.checksums, "w") as f:
            f.write("d" * 64 + "  ddl_0.2.0_linux_amd64.tar.gz\n")
        with self.assertRaises(mod.VerificationError):
            mod.verify_release_inputs("0.2.0", "v0.2.0", self.checksums)


class ScriptBehaviorTest(unittest.TestCase):
    """AC: channel updates are idempotent and verify before writing."""

    def setUp(self):
        self.tmp = tempfile.mkdtemp()
        self.checksums = os.path.join(self.tmp, "checksums.txt")
        with open(self.checksums, "w") as f:
            f.write(CHECKSUMS)

    def run_main(self, script, out_path):
        mod = load_script(script)
        rc = mod.main([script, out_path, "0.2.0", "v0.2.0", self.checksums])
        return mod, rc

    def test_homebrew_writes_formula_and_is_idempotent(self):
        out = os.path.join(self.tmp, "Formula", "dulce-de-leche.rb")
        mod, rc = self.run_main("update-homebrew.py", out)
        self.assertEqual(rc, 0)
        with open(out) as f:
            first = f.read()
        self.assertIn('version "0.2.0"', first)
        self.assertIn("ddl_0.2.0_darwin_arm64.tar.gz", first)
        self.assertIn("a" * 64, first)
        # Retry (re-run) must produce byte-identical output, not a new change.
        mod2, rc2 = self.run_main("update-homebrew.py", out)
        self.assertEqual(rc2, 0)
        with open(out) as f:
            self.assertEqual(f.read(), first)

    def test_scoop_writes_manifest_and_is_idempotent(self):
        out = os.path.join(self.tmp, "bucket", "dulce-de-leche.json")
        mod, rc = self.run_main("update-scoop.py", out)
        self.assertEqual(rc, 0)
        with open(out) as f:
            first = f.read()
        manifest = json.loads(first)
        self.assertEqual(manifest["version"], "0.2.0")
        self.assertEqual(manifest["hash"], "sha256:" + "e" * 64)
        mod2, rc2 = self.run_main("update-scoop.py", out)
        self.assertEqual(rc2, 0)
        with open(out) as f:
            self.assertEqual(f.read(), first)

    def test_homebrew_cli_fails_on_bad_tag(self):
        """The CLI path refuses mismatched tag with a non-zero exit."""
        out = os.path.join(self.tmp, "Formula", "dulce-de-leche.rb")
        proc = subprocess.run(
            [sys.executable, os.path.join(REPO, "scripts", "update-homebrew.py"),
             out, "0.2.0", "v0.1.9", self.checksums],
            capture_output=True, text=True,
        )
        self.assertNotEqual(proc.returncode, 0)
        self.assertFalse(os.path.exists(out), "refused update must not write output")

    def test_scoop_cli_fails_on_bad_tag(self):
        out = os.path.join(self.tmp, "bucket", "dulce-de-leche.json")
        proc = subprocess.run(
            [sys.executable, os.path.join(REPO, "scripts", "update-scoop.py"),
             out, "0.2.0", "v0.1.9", self.checksums],
            capture_output=True, text=True,
        )
        self.assertNotEqual(proc.returncode, 0)
        self.assertFalse(os.path.exists(out), "refused update must not write output")


class WorkflowChannelSemanticsTest(unittest.TestCase):
    """AC: disabled vs failed channels; retry idempotent; health surfaced."""

    @classmethod
    def setUpClass(cls):
        with open(RELEASE_YML) as f:
            cls.wf = yaml.safe_load(f)
        cls.publish = cls.wf["jobs"]["publish"]
        cls.steps = cls.publish["steps"]
        cls.by_name = {s.get("name"): s for s in cls.steps if "name" in s}

    def test_channel_steps_exist(self):
        self.assertIn("Update Homebrew tap", self.by_name)
        self.assertIn("Update Scoop bucket", self.by_name)

    def test_channel_steps_do_not_blindly_continue_on_error(self):
        """continue-on-error: true must not swallow real channel failures."""
        for name in ("Update Homebrew tap", "Update Scoop bucket"):
            step = self.by_name[name]
            coe = step.get("continue-on-error")
            self.assertNotEqual(
                coe, True,
                f"{name} must not use unconditional continue-on-error: true",
            )

    def test_channel_steps_distinguish_disabled_from_failed(self):
        """A missing token means the channel is intentionally disabled."""
        for name in ("Update Homebrew tap", "Update Scoop bucket"):
            run = self.by_name[name]["run"]
            self.assertIn("TAP_GITHUB_TOKEN", run)
            self.assertIn("disabled", run,
                          f"{name} must mark a missing token as disabled, not failure")

    def test_channel_steps_record_machine_readable_outcome(self):
        """Outcomes feed the release-health summary (updated/unchanged/...)."""
        for name in ("Update Homebrew tap", "Update Scoop bucket"):
            run = self.by_name[name]["run"]
            self.assertIn("CHANNEL_STATUS", run)
            self.assertIn("failed", run)

    def test_channel_steps_retry_idempotently(self):
        """Failures are retried; the retry only re-runs the channel update."""
        for name in ("Update Homebrew tap", "Update Scoop bucket"):
            run = self.by_name[name]["run"]
            self.assertIn("MAX_ATTEMPTS", run, f"{name} must retry failed updates")

    def test_release_health_step_fails_on_channel_failure(self):
        """An injected channel failure yields a visible incomplete result."""
        health = [s for s in self.steps
                  if "release-health" in s.get("id", "")
                  or "Release health" in s.get("name", "")]
        self.assertTrue(health, "publish must have a release-health summary step")
        run = health[0]["run"]
        self.assertIn("GITHUB_STEP_SUMMARY", run)
        self.assertIn("incomplete", run.lower())
        self.assertIn("exit 1", run, "release health must fail the job on channel failure")

    def test_retry_never_republishes_release_or_crate(self):
        """Retry markers must be confined to the channel steps."""
        channel_names = {"Update Homebrew tap", "Update Scoop bucket"}
        for s in self.steps:
            if s.get("name") in channel_names:
                continue
            text = yaml.safe_dump(s)
            self.assertNotIn(
                "MAX_ATTEMPTS", text,
                "only channel steps may contain the retry loop",
            )


if __name__ == "__main__":
    unittest.main()
