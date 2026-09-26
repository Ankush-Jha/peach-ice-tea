"""Ties the catalog, discount and tax modules together into a checkout total."""

from .catalog import unit_price_cents
from .discounts import discount_multiplier
from .tax import apply_tax
from .money import to_dollars


def checkout_total(items, percent=0, region="US"):
    """Compute the final checkout total, in dollars, for a cart.

    ``items`` maps SKU -> quantity. ``percent`` is a whole-number discount
    code applied to the subtotal before tax. ``region`` selects the tax rate.
    """
    subtotal_cents = sum(unit_price_cents(sku) * qty for sku, qty in items.items())
    discounted_cents = round(subtotal_cents * discount_multiplier(percent))
    total_cents = apply_tax(discounted_cents, region)
    return to_dollars(total_cents)
