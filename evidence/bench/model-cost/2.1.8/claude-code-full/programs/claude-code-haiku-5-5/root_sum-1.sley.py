def digit_sum(x: int) -> int:
    s = 0
    while x > 0:
        s += x % 10
        x //= 10
    return s


def digital_root(x: int) -> int:
    while x >= 10:
        x = digit_sum(x)
    return x


def root_sum(n: int) -> int:
    total = 0
    for i in range(1, n + 1):
        total += digital_root(i)
    return total
