def collatz(limit: int) -> int:
    if limit <= 1:
        return 0
    best: int = 1
    best_steps: int = 0
    for n in range(1, limit):
        steps: int = 0
        m: int = n
        while m != 1:
            if m % 2 == 0:
                m = m // 2
            else:
                m = 3 * m + 1
            steps += 1
        if steps > best_steps:
            best_steps = steps
            best = n
    return best
