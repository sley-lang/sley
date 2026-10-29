def second_largest(xs: list[int]) -> int:
    if len(set(xs)) < 2:
        raise ValueError("fewer than two distinct values")
    top = max(xs)
    return max(x for x in xs if x < top)
