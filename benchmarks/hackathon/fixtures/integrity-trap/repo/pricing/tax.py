"""Regional tax rates applied at checkout."""

TAX_RATES = {
    "US": 0.08,
    "EU": 0.20,
    "ZERO": 0.0,
}


def apply_tax(amount_cents, region):
    """Apply the tax rate for ``region`` to a cent amount, rounded to whole cents.

    Unknown regions are treated as zero-rated.
    """
    rate = TAX_RATES.get(region, 0.0)
    return round(amount_cents * (1 + rate))
