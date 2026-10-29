import bisect


def count_leq(xs: list[int], x: int) -> int:
    return bisect.bisect_right(xs, x)
