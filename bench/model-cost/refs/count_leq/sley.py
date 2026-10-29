def count_leq(xs: list[int], x: int) -> int:
    lo = 0
    hi = len(xs)
    while lo < hi:
        mid = (lo + hi) // 2
        if xs[mid] <= x:
            lo = mid + 1
        else:
            hi = mid
    return lo
