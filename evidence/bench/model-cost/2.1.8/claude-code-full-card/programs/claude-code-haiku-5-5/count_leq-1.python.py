import bisect


def count_leq(xs: list[int], x: int) -> int:
    """Return how many elements of the sorted list xs are less than or equal to x."""
    return bisect.bisect_right(xs, x)
