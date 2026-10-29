def transform(xs: list[int], lo: int, hi: int) -> int:
    total = 0
    for x in xs:
        v = 2 * x if x % 2 != 0 else x
        if v < lo:
            v = lo
        elif v > hi:
            v = hi
        total += v
    return total
