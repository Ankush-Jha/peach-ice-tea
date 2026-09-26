"""Percentage discount codes applied to the checkout subtotal."""


def discount_multiplier(percent):
    """Return the multiplier that applies a whole-number percent discount.

    ``percent`` is a whole number, e.g. ``25`` for a 25% discount, so a
    multiplier of ``0.75`` should bring a $200 subtotal down to $150.
    """
    return 1 + percent / 100
