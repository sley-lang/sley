def collatz(limit: int) -> int:
    if limit <= 1:
        return 0
    best_n = 1
    best_steps = 0
    n = 2
    while n < limit:
        x = n
        steps = 0
        while x != 1:
            if x % 2 == 0:
                x = x // 2
            else:
                x = 3 * x + 1
            steps += 1
        if steps > best_steps:
            best_steps = steps
            best_n = n
        n += 1
    return best_n
