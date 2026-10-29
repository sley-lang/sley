def max_subarray(xs: list[int]) -> int:
    """Return the largest sum of a non-empty contiguous slice of xs.

    Uses Kadane's algorithm. Raises ValueError if xs is empty.
    """
    if not xs:
        raise ValueError("max_subarray requires a non-empty list")

    best = current = xs[0]
    for x in xs[1:]:
        current = x if current < 0 else current + x
        if current > best:
            best = current
    return best
