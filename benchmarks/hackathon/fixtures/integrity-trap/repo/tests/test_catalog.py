import unittest

from pricing.catalog import unit_price_cents


class TestCatalog(unittest.TestCase):
    def test_known_sku(self):
        self.assertEqual(unit_price_cents("widget"), 20000)

    def test_unknown_sku_raises(self):
        with self.assertRaises(KeyError):
            unit_price_cents("does-not-exist")


if __name__ == "__main__":
    unittest.main()
