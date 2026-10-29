def second_largest(xs: list[int]) -> int:
    if len(xs) == 0:
        raise ValueError("too few values")
    top = max(xs)
    found = False
    second = 0
    for x in xs:
        if x != top and (not found or x > second):
            second = x
            found = True
    if not found:
        raise ValueError("too few values")
    return second
