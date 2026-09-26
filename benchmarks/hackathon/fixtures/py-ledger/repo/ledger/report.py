"""Plain-text rendering of an invoice."""


def render(invoice):
    rows = [f"{desc:<20}{qty:>4} x {price:>10}" for desc, price, qty in invoice.lines]
    rows.append(f"{'Subtotal':<27}{invoice.subtotal():>10}")
    if invoice.discount():
        rows.append(f"{'Discount':<27}{-invoice.discount():>10}")
    rows.append(f"{'Tax':<27}{invoice.tax():>10}")
    rows.append(f"{'Total':<27}{invoice.total():>10}")
    return "\n".join(rows)
