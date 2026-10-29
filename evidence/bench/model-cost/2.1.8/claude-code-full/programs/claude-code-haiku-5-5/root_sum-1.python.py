def root_sum(n: int) -> int:
    """Return the sum of the digital roots of the integers 1 to n inclusive."""
    total = 0
    for k in range(1, n + 1):
        total += 1 + (k - 1) % 9
    return total
