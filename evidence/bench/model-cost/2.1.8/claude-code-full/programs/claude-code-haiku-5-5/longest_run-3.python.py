def longest_run(xs: list[int]) -> int:
    best = 0
    current = 0
    prev = None
    for i, x in enumerate(xs):
        if i == 0 or x != prev:
            current = 1
        else:
            current += 1
        if current > best:
            best = current
        prev = x
    return best
