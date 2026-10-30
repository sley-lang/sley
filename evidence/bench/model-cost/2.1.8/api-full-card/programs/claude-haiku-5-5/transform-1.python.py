def transform(xs: list[int], lo: int, hi: int) -> int:
    total = 0
    for x in xs:
        y = 2 * x if x % 2 != 0 else x
        if y < lo:
            y = lo
        elif y > hi:
            y = hi
        total += y
    return total
