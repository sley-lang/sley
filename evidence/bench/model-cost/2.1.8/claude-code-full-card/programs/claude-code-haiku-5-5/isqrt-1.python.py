def isqrt(n: int) -> int:
    total = 0
    r = 0
    # For each r, the values i with r*r <= i < (r+1)*(r+1) have isqrt == r.
    # Walk r upward, adding r for every i in [r*r, min((r+1)*(r+1), n)).
    while r * r < n:
        start = r * r
        end = min((r + 1) * (r + 1), n)
        total += r * (end - start)
        r += 1
    return total
