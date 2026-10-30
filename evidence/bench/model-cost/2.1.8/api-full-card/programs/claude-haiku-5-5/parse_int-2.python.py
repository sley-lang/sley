def parse_int(chars: list[int]) -> int:
    if not chars:
        raise ValueError("empty input")

    negative = False
    start = 0
    if chars[0] == 45:
        negative = True
        start = 1

    if start >= len(chars):
        raise ValueError("no digits after sign")

    value = 0
    for code in chars[start:]:
        if not (48 <= code <= 57):
            raise ValueError(f"invalid character code: {code}")
        value = value * 10 + (code - 48)

    return -value if negative else value
