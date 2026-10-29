def mode_value(xs: list[int]) -> int:
    if len(xs) == 0:
        raise ValueError("mode_value of empty list")
    best = xs[0]
    best_count = 0
    for x in xs:
        c = 0
        for y in xs:
            if y == x:
                c += 1
        if c > best_count or (c == best_count and x < best):
            best = x
            best_count = c
    return best
