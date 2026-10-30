def second_largest(xs: list[int]) -> int:
    if xs is None:
        raise ValueError("input must be a list of integers")
    distinct = set(xs)
    if len(distinct) < 2:
        raise ValueError("fewer than two distinct values")
    distinct.remove(max(distinct))
    return max(distinct)
