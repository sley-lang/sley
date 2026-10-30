def parse_int(chars: list[int]) -> int:
    if len(chars) == 0:
        raise ValueError("empty input")
    neg = False
    start = 0
    if chars[0] == 45:
        neg = True
        start = 1
    if start == len(chars):
        raise ValueError("no digits")
    total = 0
    for i in range(start, len(chars)):
        c = chars[i]
        if c < 48 or c > 57:
            raise ValueError("invalid character")
        total = total * 10 + (c - 48)
    if neg:
        return -total
    return total
