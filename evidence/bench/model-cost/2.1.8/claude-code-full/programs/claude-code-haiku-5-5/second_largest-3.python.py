def second_largest(xs: list[int]) -> int:
    """Return the second largest distinct value in xs.

    Raises ValueError if xs has fewer than two distinct values.
    """
    distinct = set(xs)
    if len(distinct) < 2:
        raise ValueError("second_largest requires at least two distinct values")
    distinct.remove(max(distinct))
    return max(distinct)
