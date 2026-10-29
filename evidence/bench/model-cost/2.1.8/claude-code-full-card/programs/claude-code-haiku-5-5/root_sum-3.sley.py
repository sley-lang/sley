def root_sum(n: int) -> int:
    total = 0
    k = 1
    while k <= n:
        total += 1 + (k - 1) % 9
        k += 1
    return total
