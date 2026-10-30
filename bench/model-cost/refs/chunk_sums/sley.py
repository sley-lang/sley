def chunk_sums(xs: list[int], size: int) -> list[int]:
    if size < 1:
        raise ValueError("size must be at least 1")
    out = []
    i = 0
    while i < len(xs):
        total = 0
        j = i
        while j < len(xs) and j < i + size:
            total += xs[j]
            j += 1
        out.append(total)
        i += size
    return out
