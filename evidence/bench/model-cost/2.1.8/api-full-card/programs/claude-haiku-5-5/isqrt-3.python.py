import math


def isqrt(n: int) -> int:
    total = 0
    for i in range(n):
        total += math.isqrt(i)
    return total
