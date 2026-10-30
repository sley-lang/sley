def isqrt(n: int) -> int:
    total = 0
    r = 0
    i = 0
    while i < n:
        while (r + 1) * (r + 1) <= i:
            r += 1
        total += r
        i += 1
    return total
