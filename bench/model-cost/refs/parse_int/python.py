def parse_int(chars: list[int]) -> int:
    if not chars:
        raise ValueError("empty")
    sign = 1
    start = 0
    if chars[0] == 45:
        sign = -1
        start = 1
        if len(chars) == 1:
            raise ValueError("lone minus")
    value = 0
    for c in chars[start:]:
        if c < 48 or c > 57:
            raise ValueError("not a digit")
        value = value * 10 + (c - 48)
    return sign * value
