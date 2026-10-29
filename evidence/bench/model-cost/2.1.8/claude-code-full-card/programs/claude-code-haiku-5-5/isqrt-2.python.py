def isqrt(n: int) -> int:
    total = 0
    for i in range(n):
        r = 0
        while (r + 1) * (r + 1) <= i:
            r += 1
        total += r
    return total
