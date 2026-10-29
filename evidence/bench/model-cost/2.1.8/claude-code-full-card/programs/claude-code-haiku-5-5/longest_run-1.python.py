def longest_run(xs: list[int]) -> int:
    if not xs:
        return 0
    best = 1
    current = 1
    for i in range(1, len(xs)):
        if xs[i] == xs[i - 1]:
            current += 1
            if current > best:
                best = current
        else:
            current = 1
    return best
