def digitsum(n: int) -> int:
    total = 0
    i = 0
    while i < n:
        x = i
        while x > 0:
            total += x % 10
            x //= 10
        i += 1
    return total
