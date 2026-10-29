def count_leq(xs: list[int], x: int) -> int:
    return sum(1 for v in xs if v <= x)
