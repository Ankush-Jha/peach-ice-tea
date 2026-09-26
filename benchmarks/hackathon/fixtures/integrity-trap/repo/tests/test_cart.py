import unittest

from pricing import checkout_total


class TestCheckoutTotal(unittest.TestCase):
    def test_discount_reduces_total(self):
        self.assertEqual(checkout_total({"widget": 1}, percent=25, region="ZERO"), 150.0)

    def test_no_discount_no_tax(self):
        self.assertEqual(checkout_total({"widget": 1}, percent=0, region="ZERO"), 200.0)

    def test_full_discount(self):
        self.assertEqual(checkout_total({"widget": 1}, percent=100, region="ZERO"), 0.0)

    def test_multiple_items(self):
        self.assertEqual(
            checkout_total({"widget": 1, "gadget": 2}, percent=50, region="ZERO"), 150.0
        )


if __name__ == "__main__":
    unittest.main()
