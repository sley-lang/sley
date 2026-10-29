def max_subarray(xs: list[int]) -> int:
    """Return the largest sum of a non-empty contiguous slice of xs.

    Raises ValueError if xs is empty.
    """
    if not xs:
        raise ValueError("max_subarray requires a non-empty list")

    best = current = xs[0]
    for x in xs[1:]:
        current = max(x, current + x)
        if current > best:
            best = current
    return best
