def bracket_depth(s: str) -> int:
    stack: list[str] = ["?"] * len(s)
    top = 0
    best = 0
    for c in s:
        if c == "(" or c == "[" or c == "{":
            closer = ")"
            if c == "[":
                closer = "]"
            elif c == "{":
                closer = "}"
            stack[top] = closer
            top = top + 1
            best = max(best, top)
        elif c == ")" or c == "]" or c == "}":
            if top == 0 or stack[top - 1] != c:
                raise ValueError("mismatched bracket")
            top = top - 1
    if top > 0:
        raise ValueError("unclosed bracket")
    return best
