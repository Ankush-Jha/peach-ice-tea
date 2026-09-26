import unittest
from decimal import Decimal

from ledger import Invoice


class TestInvoice(unittest.TestCase):
    def test_no_discount(self):
        inv = Invoice(tax_rate="0.20").add_line("Widget", "50.00", 2)
        self.assertEqual(inv.subtotal(), Decimal("100.00"))
        self.assertEqual(inv.tax(), Decimal("20.00"))
        self.assertEqual(inv.total(), Decimal("120.00"))

    def test_tax_applies_after_discount(self):
        inv = Invoice(tax_rate="0.20", discount_rate="0.10").add_line("Widget", "100.00")
        self.assertEqual(inv.discount(), Decimal("10.00"))
        self.assertEqual(inv.tax(), Decimal("18.00"))
        self.assertEqual(inv.total(), Decimal("108.00"))

    def test_line_prices_use_half_up(self):
        inv = Invoice(tax_rate="0").add_line("Bolt", "2.665", 2)
        self.assertEqual(inv.subtotal(), Decimal("5.34"))

    def test_rejects_non_positive_quantity(self):
        with self.assertRaises(ValueError):
            Invoice(tax_rate="0").add_line("Nothing", "1.00", 0)


if __name__ == "__main__":
    unittest.main()
