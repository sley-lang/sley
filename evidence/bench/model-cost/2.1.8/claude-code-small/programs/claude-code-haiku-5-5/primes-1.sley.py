def is_prime(k: int) -> bool:
    if k < 2:
        return False
    d = 2
    while d * d <= k:
        if k % d == 0:
            return False
        d += 1
    return True


def primes(n: int) -> int:
    count = 0
    for k in range(n):
        if is_prime(k):
            count += 1
    return count
