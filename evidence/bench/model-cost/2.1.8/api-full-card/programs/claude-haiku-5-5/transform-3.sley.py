def transform(xs: list[int], lo: int, hi: int) -> int:
    total = 0
    for x in xs:
        if x % 2 == 1:
            v = 2 * x
        else:
            v = x
        total += min(max(v, lo), hi)
    return total
