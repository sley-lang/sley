def longest_run(xs: list[int]) -> int:
    best = 0
    current = 0
    previous = None
    for i, x in enumerate(xs):
        if i == 0 or x != previous:
            current = 1
        else:
            current += 1
        if current > best:
            best = current
        previous = x
    return best
