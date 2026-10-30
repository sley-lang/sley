def digitsum(n: int) -> int:
    total = 0
    i = 0
    while i < n:
        v = i
        while v > 0:
            total += v % 10
            v //= 10
        i += 1
    return total
