"""Checkout price calculations."""


def apply_discount(price, percent):
    """Apply a percentage discount to a price.

    ``percent`` is a whole number, e.g. ``25`` for a 25% discount.
    """
    return price + price * percent / 100
