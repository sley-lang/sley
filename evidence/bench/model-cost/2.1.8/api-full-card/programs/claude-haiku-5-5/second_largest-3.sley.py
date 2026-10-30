def second_largest(xs: list[int]) -> int:
    if len(set(xs)) < 2:
        raise ValueError("fewer than two distinct values")
    best = max(xs)
    second = min(xs)
    found = False
    for x in xs:
        if x < best and (not found or x > second):
            second = x
            found = True
    if not found:
        raise ValueError("fewer than two distinct values")
    return second
