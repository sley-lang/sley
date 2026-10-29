def transform(xs: list[int], lo: int, hi: int) -> int:
    total = 0
    for x in xs:
        v = x
        if x % 2 != 0:
            v = 2 * x
        if v < lo:
            v = lo
        if v > hi:
            v = hi
        total += v
    return total
