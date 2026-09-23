"""Offline checks for the release-asset/Homebrew formula contract."""

import os
from pathlib import Path
import subprocess
import tempfile
import unittest


SCRIPT = Path(__file__).with_name("generate-formula.sh")
TARGETS = (
    "aarch64-apple-darwin",
    "x86_64-apple-darwin",
    "x86_64-unknown-linux-gnu",
)


class FormulaTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.directory = Path(self.temp.name)
        for index, target in enumerate(TARGETS, start=1):
            archive = f"sqlite-tui-v0.1.0-{target}.tar.gz"
            (self.directory / f"{archive}.sha256").write_text(
                f"{str(index) * 64}  {archive}\n"
            )

    def generate(self, tag="v0.1.0", *args, env=None):
        return subprocess.run(
            ["bash", str(SCRIPT), tag, *args],
            text=True,
            capture_output=True,
            env=env,
            check=False,
        )

    def test_generates_formula_from_artifact_checksums(self):
        result = self.generate("v0.1.0", str(self.directory))
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("class SqliteTui < Formula", result.stdout)
        self.assertIn('version "0.1.0"', result.stdout)
        self.assertIn('bin.install "sqlite-tui"', result.stdout)
        for index, target in enumerate(TARGETS, start=1):
            self.assertIn(
                f"https://github.com/shonenada-vibe/sqlite-tui/releases/download/"
                f"v0.1.0/sqlite-tui-v0.1.0-{target}.tar.gz",
                result.stdout,
            )
            self.assertIn(f'sha256 "{str(index) * 64}"', result.stdout)

    def test_fetches_the_matching_published_checksums(self):
        # A fake curl keeps the test offline while checking the requested URLs.
        curl = self.directory / "curl"
        curl.write_text(
            '#!/usr/bin/env bash\nset -eu\n'
            'for url in "$@"; do :; done\n'
            'case "$url" in\n'
            '  https://github.com/shonenada-vibe/sqlite-tui/releases/download/v0.1.0/*.sha256)\n'
            '    cat "${CHECKSUM_FIXTURES}/${url##*/}" ;;\n'
            '  *) exit 1 ;;\nesac\n'
        )
        curl.chmod(0o755)
        env = {
            **os.environ,
            "PATH": f"{self.directory}{os.pathsep}{os.environ['PATH']}",
            "CHECKSUM_FIXTURES": str(self.directory),
        }
        remote = self.generate(env=env)
        local = self.generate("v0.1.0", str(self.directory))
        self.assertEqual(remote.returncode, 0, remote.stderr)
        self.assertEqual(remote.stdout, local.stdout)

    def test_rejects_invalid_tags(self):
        for tag in ("0.1.0", "v1.0.0-rc.1", 'v1.0.0";system("bad")', "v1.0.0/path"):
            with self.subTest(tag=tag):
                result = self.generate(tag, str(self.directory))
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(result.stdout, "")

    def test_rejects_missing_checksums(self):
        next(self.directory.glob("*.sha256")).unlink()
        result = self.generate("v0.1.0", str(self.directory))
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(result.stdout, "")

    def test_rejects_malformed_or_mismatched_checksums(self):
        checksum = next(self.directory.glob("*.sha256"))
        archive = checksum.name.removesuffix(".sha256")
        for text in (
            "",
            f"abc  {archive}\n",
            f"{'f' * 64}  wrong-archive.tar.gz\n",
            f"{'f' * 64}  {archive}\n{'a' * 64}  {archive}\n",
        ):
            with self.subTest(text=text):
                checksum.write_text(text)
                result = self.generate("v0.1.0", str(self.directory))
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(result.stdout, "")


if __name__ == "__main__":
    unittest.main()
