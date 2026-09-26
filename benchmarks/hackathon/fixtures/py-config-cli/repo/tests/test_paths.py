import os
import unittest

from synctool.util.paths import normalize


class TestPaths(unittest.TestCase):
    def test_collapses_parent_segments(self):
        self.assertEqual(normalize("a/b/../c"), os.path.join("a", "c"))

    def test_expands_home(self):
        self.assertEqual(normalize("~"), os.path.expanduser("~"))


if __name__ == "__main__":
    unittest.main()
