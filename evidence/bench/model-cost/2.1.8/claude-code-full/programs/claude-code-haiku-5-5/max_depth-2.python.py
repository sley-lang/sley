def max_depth(codes: list[int]) -> int:
    depth = 0
    max_seen = 0
    for code in codes:
        if code == 40:
            depth += 1
            if depth > max_seen:
                max_seen = depth
        elif code == 41:
            if depth == 0:
                raise ValueError("unbalanced parentheses: close without matching open")
            depth -= 1
    if depth != 0:
        raise ValueError("unbalanced parentheses: open never closed")
    return max_seen
