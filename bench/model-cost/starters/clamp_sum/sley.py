def clamp_sum(xs: list[int], lo: int, hi: int) -> int:
    return sum(min(max(x, lo), hi) for x in xs)
