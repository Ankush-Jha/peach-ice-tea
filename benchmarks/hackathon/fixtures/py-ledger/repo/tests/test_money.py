import unittest
from decimal import Decimal

from ledger.money import allocate, to_money


class TestToMoney(unittest.TestCase):
    def test_rounds_to_cents(self):
        self.assertEqual(to_money("1.234"), Decimal("1.23"))
        self.assertEqual(to_money(3), Decimal("3.00"))

    def test_half_cent_rounds_up(self):
        self.assertEqual(to_money("2.665"), Decimal("2.67"))
        self.assertEqual(to_money("2.675"), Decimal("2.68"))
        self.assertEqual(to_money("0.005"), Decimal("0.01"))

    def test_negative_half_cent_rounds_away_from_zero(self):
        self.assertEqual(to_money("-2.665"), Decimal("-2.67"))


class TestAllocate(unittest.TestCase):
    def test_even_split_keeps_every_cent(self):
        parts = allocate("100.00", [1, 1, 1])
        self.assertEqual(parts, [Decimal("33.34"), Decimal("33.33"), Decimal("33.33")])
        self.assertEqual(sum(parts), Decimal("100.00"))

    def test_tiny_amount(self):
        self.assertEqual(allocate("0.05", [1, 1]), [Decimal("0.03"), Decimal("0.02")])

    def test_weighted_split(self):
        parts = allocate("10.00", [1, 2])
        self.assertEqual(parts, [Decimal("3.34"), Decimal("6.66")])
        self.assertEqual(sum(parts), Decimal("10.00"))

    def test_many_parts_sum_exactly(self):
        parts = allocate("1.00", [1] * 7)
        self.assertEqual(sum(parts), Decimal("1.00"))
        self.assertEqual(parts[:2], [Decimal("0.15"), Decimal("0.15")])
        self.assertEqual(parts[2:], [Decimal("0.14")] * 5)

    def test_rejects_bad_ratios(self):
        with self.assertRaises(ValueError):
            allocate("1.00", [])
        with self.assertRaises(ValueError):
            allocate("1.00", [1, 0])


if __name__ == "__main__":
    unittest.main()
