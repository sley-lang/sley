def second_largest(xs: list[int]) -> int:
    if len(set(xs)) < 2:
        raise ValueError("fewer than two distinct values")
    first = xs[0]
    for x in xs:
        if x > first:
            first = x
    found = False
    second = 0
    for x in xs:
        if x < first:
            if not found or x > second:
                second = x
                found = True
    return second
