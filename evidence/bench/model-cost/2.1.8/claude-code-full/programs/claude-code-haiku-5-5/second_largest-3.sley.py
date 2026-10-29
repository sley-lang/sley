def second_largest(xs: list[int]) -> int:
    first: int = 0
    second: int = 0
    count: int = 0
    for x in xs:
        if count == 0:
            first = x
            count = 1
        elif x > first:
            if x != first:
                second = first
                count = count + 1 if count < 2 else count
                if count < 2:
                    count = 2
            first = x
        elif x < first and (count < 2 or x > second):
            second = x
            if count < 2:
                count = 2
    if count < 2:
        raise ValueError("fewer than two distinct values")
    return second
