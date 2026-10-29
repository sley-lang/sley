def digitsum(n: int) -> int:
    total = 0
    for i in range(n):
        for ch in str(i):
            total += int(ch)
    return total
