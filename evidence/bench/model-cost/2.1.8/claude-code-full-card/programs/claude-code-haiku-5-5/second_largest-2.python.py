def second_largest(xs: list[int]) -> int:
    """Return the second largest distinct value in xs.

    Raises ValueError if xs has fewer than two distinct values.
    """
    if not isinstance(xs, list):
        raise ValueError("input must be a list")
    distinct = set(xs)
    if len(distinct) < 2:
        raise ValueError("need at least two distinct values")
    largest = max(distinct)
    distinct.remove(largest)
    return max(distinct)
