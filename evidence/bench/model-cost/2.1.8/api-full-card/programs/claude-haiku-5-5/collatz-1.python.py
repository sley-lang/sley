def collatz(limit: int) -> int:
    if limit <= 1:
        return 0
    cache = {1: 0}

    def steps(n: int) -> int:
        path = []
        while n not in cache:
            path.append(n)
            if n % 2 == 0:
                n //= 2
            else:
                n = 3 * n + 1
        s = cache[n]
        for v in reversed(path):
            s += 1
            cache[v] = s
        return cache[path[0]] if path else s

    best_n = 1
    best_steps = 0
    for n in range(1, limit):
        s = steps(n)
        if s > best_steps:
            best_steps = s
            best_n = n
    return best_n
