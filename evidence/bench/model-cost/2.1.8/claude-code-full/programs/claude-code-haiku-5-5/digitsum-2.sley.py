def digitsum(n: int) -> int:
    total: int = 0
    for i in range(n):
        m: int = i
        while m > 0:
            total += m % 10
            m //= 10
    return total
