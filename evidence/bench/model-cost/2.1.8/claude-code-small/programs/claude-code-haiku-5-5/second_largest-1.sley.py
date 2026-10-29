def second_largest(xs: list[int]) -> int:
    if len(set(xs)) < 2:
        raise ValueError("fewer than two distinct values")
    top = max(xs)
    second = min(xs)
    found = False
    for x in xs:
        if x < top:
            if not found or x > second:
                second = x
                found = True
    return second
