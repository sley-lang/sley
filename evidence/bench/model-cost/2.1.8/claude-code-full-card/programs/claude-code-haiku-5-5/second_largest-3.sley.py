def second_largest(xs: list[int]) -> int:
    if len(set(xs)) < 2:
        raise ValueError("fewer than two distinct values")
    m = max(xs)
    best = min(xs)
    for x in xs:
        if x < m and x > best:
            best = x
    return best
