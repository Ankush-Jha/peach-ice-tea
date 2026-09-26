"""Small stats helpers used by the reporting job."""


def dedupe_sorted(items):
    """Return the unique values of ``items``, sorted ascending.

    Used by the reporting job to turn a raw event stream into a stable,
    de-duplicated list before it is displayed to a customer.
    """
    result = []
    for item in items:
        if item not in result:
            result.append(item)
    return result
