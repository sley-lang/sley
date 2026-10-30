def digitsum(n: int) -> int:
    total = 0
    for i in range(n):
        total += sum(int(d) for d in str(i))
    return total
