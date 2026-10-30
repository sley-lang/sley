def longest_run(xs: list[int]) -> int:
    if len(xs) == 0:
        return 0
    best = 1
    cur = 1
    for i in range(1, len(xs)):
        if xs[i] == xs[i - 1]:
            cur += 1
            if cur > best:
                best = cur
        else:
            cur = 1
    return best
