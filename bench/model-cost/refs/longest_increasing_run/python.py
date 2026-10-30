def longest_increasing_run(xs: list[int]) -> int:
    best = 0
    run = 0
    for i, x in enumerate(xs):
        if i > 0 and x > xs[i - 1]:
            run += 1
        else:
            run = 1
        best = max(best, run)
    return best
