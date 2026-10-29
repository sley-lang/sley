def root_sum(n: int) -> int:
    q = n // 9
    r = n % 9
    return 45 * q + r * (r + 1) // 2
