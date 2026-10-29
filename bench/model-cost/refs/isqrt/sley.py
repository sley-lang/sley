def isqrt(n: int) -> int:
    total = 0
    for i in range(n):
        lo = 0
        hi = i + 1
        while hi - lo > 1:
            mid = (lo + hi) // 2
            if mid * mid <= i:
                lo = mid
            else:
                hi = mid
        total += lo
    return total
