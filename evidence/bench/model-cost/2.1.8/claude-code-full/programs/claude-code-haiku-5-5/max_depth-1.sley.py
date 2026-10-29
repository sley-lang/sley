def max_depth(codes: list[int]) -> int:
    depth: int = 0
    best: int = 0
    for c in codes:
        if c == 40:
            depth += 1
            if depth > best:
                best = depth
        elif c == 41:
            if depth == 0:
                raise ValueError("unbalanced parentheses: close without open")
            depth -= 1
    if depth != 0:
        raise ValueError("unbalanced parentheses: open never closed")
    return best
