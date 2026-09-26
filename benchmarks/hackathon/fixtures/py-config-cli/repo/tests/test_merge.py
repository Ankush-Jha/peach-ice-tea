import unittest

from synctool.config.defaults import DEFAULTS
from synctool.config.merge import merge_settings


class TestMerge(unittest.TestCase):
    def test_defaults_only(self):
        self.assertEqual(merge_settings({}, {"command": "status", "config": None}), DEFAULTS)

    def test_file_overrides_defaults(self):
        merged = merge_settings({"retries": 5}, {"command": "status", "config": "x"})
        self.assertEqual(merged["retries"], 5)

    def test_cli_overrides_file(self):
        merged = merge_settings({"retries": 5}, {"retries": 8, "command": "status", "config": "x"})
        self.assertEqual(merged["retries"], 8)

    def test_unset_cli_values_do_not_override(self):
        merged = merge_settings({"verbose": True}, {"verbose": None, "command": "status", "config": "x"})
        self.assertIs(merged["verbose"], True)


if __name__ == "__main__":
    unittest.main()
