def collatz_steps(n: int) -> int:
    steps = 0
    while n != 1:
        if n % 2 == 0:
            n = n // 2
        else:
            n = 3 * n + 1
        steps += 1
    return steps


def collatz(limit: int) -> int:
    if limit <= 1:
        return 0
    best = 1
    best_steps = 0
    for n in range(1, limit):
        s = collatz_steps(n)
        if s > best_steps:
            best = n
            best_steps = s
    return best
