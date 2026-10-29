def transform(xs: list[int], lo: int, hi: int) -> int:
    return sum(min(max(x * 2 if x % 2 else x, lo), hi) for x in xs)
