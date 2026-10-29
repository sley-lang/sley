def root_sum(n: int) -> int:
    total = 0
    for k in range(1, n + 1):
        d = k
        while d >= 10:
            s = 0
            while d:
                s += d % 10
                d //= 10
            d = s
        total += d
    return total
