import unittest

from pricing.tax import apply_tax


class TestApplyTax(unittest.TestCase):
    def test_us_rate(self):
        self.assertEqual(apply_tax(10000, "US"), 10800)

    def test_zero_rate_for_unknown_region(self):
        self.assertEqual(apply_tax(10000, "made-up"), 10000)


if __name__ == "__main__":
    unittest.main()
