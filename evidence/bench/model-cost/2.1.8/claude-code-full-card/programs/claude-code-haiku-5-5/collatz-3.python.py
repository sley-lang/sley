def collatz(limit: int) -> int:
    if limit <= 1:
        return 0

    # steps[v] holds the number of steps for v to reach 1, for every v < limit.
    steps = [0] * limit
    best_n, best_steps = 1, 0

    for n in range(2, limit):
        x = n
        count = 0
        # Walk until we drop below n, where the step count is already known.
        while x >= n:
            x = x // 2 if x % 2 == 0 else 3 * x + 1
            count += 1
        count += steps[x]
        steps[n] = count
        # Strict comparison keeps the smallest n on ties.
        if count > best_steps:
            best_n, best_steps = n, count

    return best_n
