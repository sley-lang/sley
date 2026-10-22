def gcd(a: int, b: int) -> int:
    while b != 0:
        a, b = b, a % b
    return a


def gcdsum(n: int) -> int:
    s = 0
    for i in range(1, n):
        for j in range(1, n):
            s += gcd(i, j)
    return s
