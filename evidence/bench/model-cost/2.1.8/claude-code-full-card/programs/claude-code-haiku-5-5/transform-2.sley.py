def transform(xs: list[int], lo: int, hi: int) -> int:
    total = 0
    for x in xs:
        v = 2 * x if x % 2 != 0 else x
        total += min(max(v, lo), hi)
    return total
