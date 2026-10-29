def max_subarray(xs: list[int]) -> int:
    if len(xs) == 0:
        raise ValueError("max_subarray: empty input")
    best = xs[0]
    cur = xs[0]
    for i in range(1, len(xs)):
        x = xs[i]
        cur = max(x, cur + x)
        best = max(best, cur)
    return best
