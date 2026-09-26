"""Money helpers. All amounts are Decimal values quantized to whole cents."""

from decimal import ROUND_HALF_EVEN, Decimal

CENT = Decimal("0.01")


def to_money(value):
    """Convert a number or numeric string to a Decimal rounded to cents."""
    return Decimal(str(value)).quantize(CENT, rounding=ROUND_HALF_EVEN)


def allocate(amount, ratios):
    """Split `amount` across `ratios` (positive numbers), one part per ratio."""
    if not ratios or any(r <= 0 for r in ratios):
        raise ValueError("ratios must be a non-empty list of positive numbers")
    amount = to_money(amount)
    total = sum(Decimal(str(r)) for r in ratios)
    return [to_money(amount * Decimal(str(r)) / total) for r in ratios]
