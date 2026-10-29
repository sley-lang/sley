def max_subarray(xs: list[int]) -> int:
    """Return the largest sum of a non-empty contiguous slice of xs.

    Raises ValueError if xs is empty.
    """
    if not isinstance(xs, (list, tuple)):
        raise ValueError("xs must be a list of integers")
    if len(xs) == 0:
        raise ValueError("max_subarray requires a non-empty sequence")

    best = xs[0]
    current = xs[0]
    for x in xs[1:]:
        current = x if current < 0 else current + x
        if current > best:
            best = current
    return best
