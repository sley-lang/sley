def primes(n: int) -> int:
    count = 0
    for k in range(2, n):
        d = 2
        is_p = True
        while d * d <= k:
            if k % d == 0:
                is_p = False
                break
            d += 1
        if is_p:
            count += 1
    return count
