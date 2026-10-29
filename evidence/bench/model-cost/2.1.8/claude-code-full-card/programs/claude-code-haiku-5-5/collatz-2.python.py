def collatz(limit: int) -> int:
    if limit <= 1:
        return 0

    # Memoized step counts; cache[v] = number of steps for v to reach 1.
    cache = {1: 0}
    best_n = 1
    best_steps = 0

    for n in range(2, limit):
        # Walk forward until hitting a value with a known step count.
        path = []
        x = n
        while x not in cache:
            path.append(x)
            x = x // 2 if x % 2 == 0 else 3 * x + 1

        # Fill in step counts on the way back.
        steps = cache[x]
        for v in reversed(path):
            steps += 1
            cache[v] = steps

        # Strictly greater keeps the smallest n on ties (ascending scan).
        if cache[n] > best_steps:
            best_steps = cache[n]
            best_n = n

    return best_n
