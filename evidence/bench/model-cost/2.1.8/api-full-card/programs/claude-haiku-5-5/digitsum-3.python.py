def digitsum(n: int) -> int:
    total = 0
    for i in range(n):
        total += sum(int(c) for c in str(i))
    return total
