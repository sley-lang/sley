def max_subarray(xs: list[int]) -> int:
    if len(xs) == 0:
        raise ValueError("empty")
    best = xs[0]
    cur = xs[0]
    for i in range(1, len(xs)):
        cur = max(xs[i], cur + xs[i])
        best = max(best, cur)
    return best
