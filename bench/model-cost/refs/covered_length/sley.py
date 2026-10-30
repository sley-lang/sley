def covered_length(xs: list[int]) -> int:
    # No sorting in the dialect: for each distinct endpoint c, find the next
    # larger endpoint and add the gap if some interval covers all of it.
    n = len(xs)
    if n % 2 != 0:
        raise ValueError("odd length")
    for i in range(0, n, 2):
        if xs[i] > xs[i + 1]:
            raise ValueError("start > end")
    total = 0
    for i in range(n):
        c = xs[i]
        seen = False
        for j in range(i):
            if xs[j] == c:
                seen = True
        if not seen:
            nxt = c
            found = False
            for j in range(n):
                if xs[j] > c and (not found or xs[j] < nxt):
                    nxt = xs[j]
                    found = True
            if found:
                covered = False
                for k in range(0, n, 2):
                    if xs[k] <= c and xs[k + 1] >= nxt:
                        covered = True
                if covered:
                    total += nxt - c
    return total
