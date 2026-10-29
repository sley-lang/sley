def max_depth(codes: list[int]) -> int:
    depth = 0
    best = 0
    for code in codes:
        if code == 40:
            depth += 1
            if depth > best:
                best = depth
        elif code == 41:
            if depth == 0:
                raise ValueError("unbalanced parentheses: close without open")
            depth -= 1
    if depth != 0:
        raise ValueError("unbalanced parentheses: open never closed")
    return best
