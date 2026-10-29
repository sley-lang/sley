def digitsum(n: int) -> int:
    total: int = 0
    i: int = 0
    while i < n:
        x: int = i
        while x > 0:
            total += x % 10
            x //= 10
        i += 1
    return total
