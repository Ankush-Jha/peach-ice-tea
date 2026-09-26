"""End to end: run synctool as a separate process against a real config file."""

import os
import subprocess
import sys
import tempfile
import unittest

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def synctool(*args):
    result = subprocess.run(
        [sys.executable, "-m", "synctool", *args],
        cwd=REPO,
        capture_output=True,
        text=True,
        timeout=30,
    )
    return result.returncode, result.stdout.strip()


class TestProcess(unittest.TestCase):
    def test_config_file_drives_a_real_run(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = os.path.join(tmp, "sync.conf")
            with open(path, "w", encoding="utf-8") as handle:
                handle.write("# team defaults\nverbose = true\nretries = 5\ndry-run = true\n")
            self.assertEqual(synctool("--config", path, "status"), (0, "verbose=True retries=5 dry_run=True"))
            self.assertEqual(synctool("--config", path, "--retries", "1", "status"), (0, "verbose=True retries=1 dry_run=True"))
            self.assertEqual(synctool("--config", path, "sync"), (0, "would sync ."))

    def test_no_config_uses_defaults(self):
        self.assertEqual(synctool("status"), (0, "verbose=False retries=3 dry_run=False"))


if __name__ == "__main__":
    unittest.main()
