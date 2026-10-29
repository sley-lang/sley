def mode_value(xs: list[int]) -> int:
    if len(xs) == 0:
        raise ValueError("mode_value: empty list")
    best = xs[0]
    best_count = 0
    for x in xs:
        c = sum(1 for y in xs if y == x)
        if c > best_count or (c == best_count and x < best):
            best = x
            best_count = c
    return best
