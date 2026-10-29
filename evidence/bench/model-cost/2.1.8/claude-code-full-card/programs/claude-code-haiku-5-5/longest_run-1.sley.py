def longest_run(xs: list[int]) -> int:
    best = 0
    cur = 0
    for i in range(len(xs)):
        if i == 0 or xs[i] != xs[i - 1]:
            cur = 1
        else:
            cur += 1
        if cur > best:
            best = cur
    return best
