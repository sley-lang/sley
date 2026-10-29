def collatz(limit: int) -> int:
    if limit <= 1:
        return 0

    # memo[v] = number of steps for the Collatz sequence of v to reach 1
    memo = {1: 0}
    best_n = 1
    best_steps = 0

    for n in range(1, limit):
        # Walk forward until reaching a value whose step count is known
        path = []
        x = n
        while x not in memo:
            path.append(x)
            x = x // 2 if x % 2 == 0 else 3 * x + 1

        # Fill in step counts for every value on the path, back to front
        s = memo[x]
        for v in reversed(path):
            s += 1
            memo[v] = s

        steps = memo[n]
        # Strict comparison keeps the smallest n on ties
        if steps > best_steps:
            best_n = n
            best_steps = steps

    return best_n
