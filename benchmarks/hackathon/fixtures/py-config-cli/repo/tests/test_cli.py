import io
import os
import tempfile
import unittest

from synctool.cli import main


def run(argv, config_text=None):
    out = io.StringIO()
    if config_text is None:
        code = main(argv, out)
        return code, out.getvalue().strip()
    with tempfile.TemporaryDirectory() as tmp:
        path = os.path.join(tmp, "sync.conf")
        with open(path, "w", encoding="utf-8") as handle:
            handle.write(config_text)
        code = main(["--config", path, *argv], out)
    return code, out.getvalue().strip()


class TestCli(unittest.TestCase):
    def test_defaults_without_config(self):
        self.assertEqual(run(["status"]), (0, "verbose=False retries=3 dry_run=False"))

    def test_flags_without_config(self):
        self.assertEqual(
            run(["--verbose", "--retries", "9", "status"]),
            (0, "verbose=True retries=9 dry_run=False"),
        )

    def test_config_file_is_used(self):
        self.assertEqual(
            run(["status"], "verbose = true\nretries = 5\n"),
            (0, "verbose=True retries=5 dry_run=False"),
        )

    def test_command_line_beats_config_file(self):
        self.assertEqual(
            run(["--retries", "7", "status"], "verbose = true\nretries = 5\n"),
            (0, "verbose=True retries=7 dry_run=False"),
        )

    def test_explicit_default_value_on_command_line_still_wins(self):
        self.assertEqual(
            run(["--retries", "3", "status"], "retries = 5\n"),
            (0, "verbose=False retries=3 dry_run=False"),
        )

    def test_dash_keys_in_config(self):
        self.assertEqual(
            run(["sync"], "dry-run = true\n"),
            (0, "would sync ."),
        )


if __name__ == "__main__":
    unittest.main()
