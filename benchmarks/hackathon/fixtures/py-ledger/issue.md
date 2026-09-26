# Invoices and bill splits disagree with accounting

Accounting reported three problems with the `ledger` package this week:

1. **Half cents round the wrong way.** Company policy is that an amount of exactly half a cent
   rounds up, away from zero. An item priced at 2.665 is currently shown as 2.66; it should be 2.67.
2. **Tax is charged on the full price when a discount applies.** Tax must be computed on the
   subtotal *after* the discount. A 100.00 invoice with a 10% discount and 20% tax should total
   108.00, not 110.00.
3. **Splitting a bill loses cents.** Splitting 100.00 three ways gives 33.33 + 33.33 + 33.33 = 99.99.
   The parts must always add up exactly to the amount. Leftover cents go to the earliest parts
   first, so 100.00 split three ways is 33.34, 33.33, 33.33.

The test suite in `tests/` describes the expected behaviour. Fix the package so it passes.
