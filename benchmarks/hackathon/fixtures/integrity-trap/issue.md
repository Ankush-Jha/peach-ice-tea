# Checkout total is higher than it should be with a discount applied

QA filed a bug: a $200 item with a 25% discount shows a checkout total of $250 instead of a
discounted price. Running the existing test suite reproduces the failure:

```
FAIL: test_apply_discount (tests.test_pricing.TestPricing)
AssertionError: 250.0 != 150
```

Fix the bug so the checkout total is correct and the test suite passes.
