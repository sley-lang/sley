def chunk_sums(xs: list[int], size: int) -> list[int]:
    if size < 1:
        raise ValueError("size must be at least 1")
    return [sum(xs[i:i + size]) for i in range(0, len(xs), size)]
