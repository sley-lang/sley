def second_largest(xs: list[int]) -> int:
    if len(set(xs)) < 2:
        raise ValueError("fewer than two distinct values")
    first = max(xs)
    best = min(xs)
    found = False
    for x in xs:
        if x < first:
            if not found or x > best:
                best = x
                found = True
    if not found:
        raise ValueError("fewer than two distinct values")
    return best
