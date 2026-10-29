def collatz(limit: int) -> int:
    if limit <= 1:
        return 0

    # Memoized step counts: cache[v] = number of steps for v to reach 1.
    cache = {1: 0}
    best_n, best_steps = 1, 0

    for start in range(2, limit):
        path = []
        n = start
        while n not in cache:
            path.append(n)
            n = n // 2 if n % 2 == 0 else 3 * n + 1

        steps = cache[n]
        for v in reversed(path):
            steps += 1
            cache[v] = steps

        if cache[start] > best_steps:
            best_n, best_steps = start, cache[start]

    return best_n
