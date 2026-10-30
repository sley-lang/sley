def clamp_sum(xs: list[int], lo: int, hi: int) -> int:
    if lo > hi:
        raise ValueError("lo must not exceed hi")
    return sum(min(max(x, lo), hi) for x in xs if x >= 0)
