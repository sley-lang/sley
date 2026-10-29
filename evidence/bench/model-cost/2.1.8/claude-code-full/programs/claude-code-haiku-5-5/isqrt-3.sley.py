def isqrt(n: int) -> int:
    total: int = 0
    r: int = 0
    for i in range(n):
        while (r + 1) * (r + 1) <= i:
            r += 1
        total += r
    return total
