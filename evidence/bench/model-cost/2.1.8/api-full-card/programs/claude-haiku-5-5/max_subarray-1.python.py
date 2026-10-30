def max_subarray(xs: list[int]) -> int:
    if not xs:
        raise ValueError("max_subarray requires a non-empty list")
    best = cur = xs[0]
    for x in xs[1:]:
        cur = x if cur < 0 else cur + x
        if cur > best:
            best = cur
    return best
