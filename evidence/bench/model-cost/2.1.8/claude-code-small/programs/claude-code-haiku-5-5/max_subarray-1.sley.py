def max_subarray(xs: list[int]) -> int:
    if len(xs) == 0:
        raise ValueError("max_subarray: empty list")
    cur = xs[0]
    best = xs[0]
    for i in range(1, len(xs)):
        x = xs[i]
        cur = max(cur + x, x)
        best = max(best, cur)
    return best
