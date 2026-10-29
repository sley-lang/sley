def primes(n: int) -> int:
    count = 0
    k = 2
    while k < n:
        is_p = True
        d = 2
        while d * d <= k:
            if k % d == 0:
                is_p = False
                break
            d += 1
        if is_p:
            count += 1
        k += 1
    return count
