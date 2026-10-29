def root_sum(n: int) -> int:
    total: int = 0
    for k in range(1, n + 1):
        total += 1 + (k - 1) % 9
    return total
