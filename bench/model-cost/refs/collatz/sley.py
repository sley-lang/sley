def collatz(limit: int) -> int:
    best = -1
    best_n = 0
    for n in range(1, limit):
        x = n
        steps = 0
        while x != 1:
            if x % 2 == 0:
                x = x // 2
            else:
                x = 3 * x + 1
            steps += 1
        if steps > best:
            best = steps
            best_n = n
    return best_n
