def max_subarray(xs: list[int]) -> int:
    if len(xs) == 0:
        raise ValueError("max_subarray: empty input")
    best = xs[0]
    cur = xs[0]
    for i in range(1, len(xs)):
        v = xs[i]
        cur = max(v, cur + v)
        best = max(best, cur)
    return best
