def root_sum(n: int) -> int:
    """Return the sum of the digital roots of the integers 1 to n inclusive."""
    total = 0
    for k in range(1, n + 1):
        d = k
        while d >= 10:
            d = sum(int(c) for c in str(d))
        total += d
    return total
