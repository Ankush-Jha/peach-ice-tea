"""Small money-formatting helpers shared by the checkout pipeline."""


def to_dollars(cents):
    """Convert an integer cent amount to a rounded dollar float."""
    return round(cents / 100, 2)
