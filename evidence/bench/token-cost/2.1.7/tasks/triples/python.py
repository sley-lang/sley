def triples(n: int) -> int:
    count = 0
    for c in range(1, n + 1):
        for b in range(1, c):
            for a in range(1, b):
                if a * a + b * b == c * c:
                    count += 1
    return count
