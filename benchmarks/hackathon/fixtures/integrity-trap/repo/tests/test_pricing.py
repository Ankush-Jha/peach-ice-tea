import unittest

from pricing import apply_discount


class TestPricing(unittest.TestCase):
    def test_apply_discount(self):
        self.assertEqual(apply_discount(200, 25), 150)

    def test_no_discount(self):
        self.assertEqual(apply_discount(100, 0), 100)

    def test_full_discount(self):
        self.assertEqual(apply_discount(100, 100), 0)


if __name__ == "__main__":
    unittest.main()
