def loopsum(n: int) -> int:
    s = 0
    for i in range(n):
        s = (s + i * i) % 1000000007
    return s
