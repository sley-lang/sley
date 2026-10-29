def root_sum(n: int) -> int:
    return sum(1 + (i - 1) % 9 for i in range(1, n + 1))
