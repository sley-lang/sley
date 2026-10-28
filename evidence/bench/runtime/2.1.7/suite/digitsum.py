def digitsum(n: int) -> int:
    total = 0
    for i in range(n):
        x = i
        while x > 0:
            total += x % 10
            x //= 10
    return total
