def second_largest_distinct(xs: list[int]) -> int:
    distinct = sorted(set(xs))
    if len(distinct) < 2:
        raise ValueError("fewer than two distinct values")
    return distinct[-2]
