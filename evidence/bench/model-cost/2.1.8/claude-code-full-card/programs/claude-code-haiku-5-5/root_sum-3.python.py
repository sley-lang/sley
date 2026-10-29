def root_sum(n: int) -> int:
    """Return the sum of digital roots of the integers 1 to n inclusive.

    The digital root of a positive integer k is 1 + (k - 1) % 9.
    For n == 0 the sum is empty and therefore 0.
    """
    if n <= 0:
        return 0
    total = 0
    for k in range(1, n + 1):
        total += 1 + (k - 1) % 9
    return total
