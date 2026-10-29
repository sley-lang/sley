def collatz(limit: int) -> int:
    if limit <= 1:
        return 0

    # steps[k] holds the number of Collatz steps for k, for every k < limit.
    steps = [0] * limit  # steps[1] == 0
    best_n, best_steps = 1, 0

    for n in range(2, limit):
        # Walk until the value drops below n, then reuse the cached result.
        x = n
        count = 0
        while x >= n:
            x = x // 2 if x % 2 == 0 else 3 * x + 1
            count += 1
        total = count + steps[x]
        steps[n] = total
        # Strict comparison keeps the smallest n on ties.
        if total > best_steps:
            best_steps = total
            best_n = n

    return best_n
