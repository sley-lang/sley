def root_sum(n: int) -> int:
    total = 0
    for i in range(1, n + 1):
        total += 1 + (i - 1) % 9
    return total
