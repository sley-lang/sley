def collatz(limit: int) -> int:
    if limit <= 1:
        return 0

    # known[v] = number of steps for v to reach 1, for v < limit; -1 if unknown.
    known = [-1] * limit
    known[1] = 0

    best_n = 1
    best_steps = 0

    for n in range(2, limit):
        path = []
        x = n
        # Walk until reaching a value with known step count (all values < limit).
        while x >= limit or known[x] < 0:
            path.append(x)
            if x % 2 == 0:
                x //= 2
            else:
                x = 3 * x + 1
        s = known[x] if x < limit else 0
        # Assign step counts back along the path.
        for y in reversed(path):
            s += 1
            if y < limit:
                known[y] = s
        steps = s
        if steps > best_steps:
            best_steps = steps
            best_n = n

    return best_n
