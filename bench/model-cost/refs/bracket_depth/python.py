PAIRS = {")": "(", "]": "[", "}": "{"}


def bracket_depth(s: str) -> int:
    stack = []
    best = 0
    for c in s:
        if c in "([{":
            stack.append(c)
            best = max(best, len(stack))
        elif c in PAIRS:
            if not stack or stack[-1] != PAIRS[c]:
                raise ValueError("mismatched bracket")
            stack.pop()
    if stack:
        raise ValueError("unclosed bracket")
    return best
