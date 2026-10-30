def digitsum(n: int) -> int:
    total = 0
    i = 0
    while i < n:
        m = i
        while m > 0:
            total += m % 10
            m //= 10
        i += 1
    return total
