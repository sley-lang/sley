def isqrt(n: int) -> int:
    # For each r >= 0, the integers i with floor(sqrt(i)) == r are
    # exactly those in [r*r, (r+1)*(r+1)). Sum r over the part of that
    # range that lies below n.
    total = 0
    r = 0
    while r * r < n:
        start = r * r
        end = min((r + 1) * (r + 1), n)
        total += r * (end - start)
        r += 1
    return total
