def longest_run(xs: list[int]) -> int:
    best = 0
    cur = 0
    prev = 0
    for i, x in enumerate(xs):
        if i == 0 or x != prev:
            cur = 1
        else:
            cur += 1
        if cur > best:
            best = cur
        prev = x
    return best
