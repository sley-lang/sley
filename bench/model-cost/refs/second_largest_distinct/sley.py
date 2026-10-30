def second_largest_distinct(xs: list[int]) -> int:
    if len(xs) == 0:
        raise ValueError("empty")
    top = max(xs)
    found = False
    best = 0
    for x in xs:
        if x < top and (not found or x > best):
            best = x
            found = True
    if not found:
        raise ValueError("fewer than two distinct values")
    return best
