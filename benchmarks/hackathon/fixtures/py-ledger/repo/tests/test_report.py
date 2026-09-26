import unittest

from ledger import Invoice
from ledger.report import render


class TestReport(unittest.TestCase):
    def test_render_without_discount(self):
        inv = Invoice(tax_rate="0.10").add_line("Widget", "10.00", 3)
        lines = render(inv).splitlines()
        self.assertEqual(len(lines), 4)
        self.assertTrue(lines[0].startswith("Widget"))
        self.assertTrue(lines[-1].endswith("33.00"))

    def test_render_with_discount_shows_discount_line(self):
        inv = Invoice(tax_rate="0.20", discount_rate="0.10").add_line("Widget", "100.00")
        text = render(inv)
        self.assertIn("Discount", text)
        self.assertTrue(text.splitlines()[-1].endswith("108.00"))


if __name__ == "__main__":
    unittest.main()
