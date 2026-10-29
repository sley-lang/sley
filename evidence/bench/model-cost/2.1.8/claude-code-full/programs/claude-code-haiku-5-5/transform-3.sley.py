def transform(xs: list[int], lo: int, hi: int) -> int:
    total: int = 0
    for x in xs:
        v: int = 2 * x if x % 2 != 0 else x
        c: int = min(max(v, lo), hi)
        total += c
    return total
