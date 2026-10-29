def longest_run(xs: list[int]) -> int:
    best = 0
    run = 0
    for i in range(len(xs)):
        if i > 0 and xs[i] == xs[i - 1]:
            run += 1
        else:
            run = 1
        if run > best:
            best = run
    return best
