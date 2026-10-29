def is_prime(k: int) -> bool:
    d = 2
    while d * d <= k:
        if k % d == 0:
            return False
        d += 1
    return True


def primes(n: int) -> int:
    return sum(1 for k in range(2, n) if is_prime(k))
