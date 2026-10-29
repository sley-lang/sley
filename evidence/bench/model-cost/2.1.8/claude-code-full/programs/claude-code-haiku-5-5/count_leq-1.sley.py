def count_leq(xs: list[int], x: int) -> int:
    lo: int = 0
    hi: int = len(xs)
    while lo < hi:
        mid: int = (lo + hi) // 2
        if xs[mid] <= x:
            lo = mid + 1
        else:
            hi = mid
    return lo
