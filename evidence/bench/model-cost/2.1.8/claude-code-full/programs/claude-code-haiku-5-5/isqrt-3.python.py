import math


def isqrt(n: int) -> int:
    """Return the sum of floor(sqrt(i)) for all integers 0 <= i < n."""
    total = 0
    for i in range(n):
        total += math.isqrt(i)
    return total
