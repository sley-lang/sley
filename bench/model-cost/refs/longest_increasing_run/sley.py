def longest_increasing_run(xs: list[int]) -> int:
    best = 0
    run = 0
    for i in range(len(xs)):
        if i > 0 and xs[i - 1] < xs[i]:
            run += 1
        else:
            run = 1
        best = max(best, run)
    return best
