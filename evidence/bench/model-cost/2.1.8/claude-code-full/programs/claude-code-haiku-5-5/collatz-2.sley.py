def collatz_steps(n: int) -> int:
    steps: int = 0
    while n != 1:
        if n % 2 == 0:
            n //= 2
        else:
            n = 3 * n + 1
        steps += 1
    return steps


def collatz(limit: int) -> int:
    if limit <= 1:
        return 0
    best: int = 1
    best_steps: int = 0
    n: int = 1
    while n < limit:
        s: int = collatz_steps(n)
        if s > best_steps:
            best_steps = s
            best = n
        n += 1
    return best
