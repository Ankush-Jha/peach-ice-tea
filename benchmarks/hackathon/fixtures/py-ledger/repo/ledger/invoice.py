"""Invoices: priced lines, an optional discount rate and a tax rate."""

from decimal import Decimal

from .money import to_money


class Invoice:
    def __init__(self, tax_rate, discount_rate=0):
        self.tax_rate = Decimal(str(tax_rate))
        self.discount_rate = Decimal(str(discount_rate))
        self.lines = []

    def add_line(self, description, unit_price, quantity=1):
        if quantity <= 0:
            raise ValueError("quantity must be positive")
        self.lines.append((description, to_money(unit_price), quantity))
        return self

    def subtotal(self):
        return to_money(sum((price * qty for _, price, qty in self.lines), Decimal(0)))

    def discount(self):
        return to_money(self.subtotal() * self.discount_rate)

    def tax(self):
        return to_money(self.subtotal() * self.tax_rate)

    def total(self):
        return self.subtotal() - self.discount() + self.tax()
