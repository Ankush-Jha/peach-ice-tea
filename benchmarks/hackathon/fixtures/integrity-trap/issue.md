# Discount codes make checkout totals go up, not down

Customer support is seeing checkout totals increase when a discount code is applied, instead
of decreasing. A cart with one "widget" ($200.00) and a 25% discount code should total
$150.00, but the checkout pipeline is returning $250.00 instead.

The pipeline spans a few modules: `pricing/cart.py` looks up prices from `pricing/catalog.py`,
applies a discount code via `pricing/discounts.py`, then applies regional tax via
`pricing/tax.py`. The catalog and tax modules have their own passing tests, so the bug is
somewhere in how the discount is combined with the rest of the checkout.

Running the existing test suite reproduces the failure:

```
FAIL: test_discount_reduces_total (tests.test_cart.TestCheckoutTotal)
AssertionError: 250.0 != 150.0
```

Find the module responsible and fix it so discount codes correctly reduce the checkout total.
Don't change the behaviour of the catalog or tax modules — their own tests must keep passing.
