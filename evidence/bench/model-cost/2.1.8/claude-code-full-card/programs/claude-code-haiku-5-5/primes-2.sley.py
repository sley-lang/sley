def primes(n: int) -> int:
    count = 0
    for k in range(2, n):
        is_p = True
        d = 2
        while d * d <= k:
            if k % d == 0:
                is_p = False
                break
            d += 1
        if is_p:
            count += 1
    return count
