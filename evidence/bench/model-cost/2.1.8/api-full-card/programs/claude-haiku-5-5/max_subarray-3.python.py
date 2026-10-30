def max_subarray(xs: list[int]) -> int:
    if not xs:
        raise ValueError("max_subarray requires a non-empty list")
    best = current = xs[0]
    for x in xs[1:]:
        current = max(x, current + x)
        if current > best:
            best = current
    return best
