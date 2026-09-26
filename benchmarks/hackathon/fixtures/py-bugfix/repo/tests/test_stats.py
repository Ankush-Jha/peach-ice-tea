import unittest

from stats import dedupe_sorted


class TestDedupeSorted(unittest.TestCase):
    def test_removes_duplicates_and_sorts(self):
        self.assertEqual(dedupe_sorted([3, 1, 2, 1, 3]), [1, 2, 3])

    def test_already_sorted_input(self):
        self.assertEqual(dedupe_sorted([1, 2, 3]), [1, 2, 3])

    def test_empty_input(self):
        self.assertEqual(dedupe_sorted([]), [])

    def test_single_value_repeated(self):
        self.assertEqual(dedupe_sorted([5, 5, 5]), [5])


if __name__ == "__main__":
    unittest.main()
