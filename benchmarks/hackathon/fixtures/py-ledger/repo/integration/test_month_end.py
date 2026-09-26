"""End to end: a month-end invoice, its printed report, and the split between payers."""

import unittest
from decimal import Decimal

from ledger import Invoice, allocate
from ledger.report import render


class TestMonthEnd(unittest.TestCase):
    def test_discounted_invoice_is_rendered_and_split_to_the_cent(self):
        invoice = (
            Invoice(tax_rate="0.20", discount_rate="0.10")
            .add_line("Hosting", "49.995", 1)
            .add_line("Support hours", "30.00", 2)
        )
        total = invoice.total()
        text = render(invoice)
        shares = allocate(total, [1, 1, 1])

        self.assertEqual(invoice.subtotal(), Decimal("110.00"))
        self.assertEqual(total, Decimal("118.80"))
        self.assertTrue(text.splitlines()[-1].endswith("118.80"))
        self.assertEqual(shares, [Decimal("39.60")] * 3)
        self.assertEqual(sum(allocate("100.00", [3, 3, 3])), Decimal("100.00"))


if __name__ == "__main__":
    unittest.main()
