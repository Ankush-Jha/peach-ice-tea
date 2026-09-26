"""Product catalog: SKU -> unit price in cents.

Prices are stored in cents so totals can be summed across many items without
floating-point rounding error creeping in before tax is applied.
"""

CATALOG = {
    "widget": 20000,
    "gadget": 5000,
}


def unit_price_cents(sku):
    """Look up the unit price of ``sku`` in cents.

    Raises ``KeyError`` if the SKU is not in the catalog.
    """
    return CATALOG[sku]
