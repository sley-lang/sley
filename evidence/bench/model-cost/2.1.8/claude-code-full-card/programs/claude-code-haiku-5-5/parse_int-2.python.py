def parse_int(chars: list[int]) -> int:
    """Parse a list of character codes as a decimal integer with an optional leading minus.

    Raises ValueError if chars is empty, is only a minus sign, or contains any
    code other than a digit (48-57) or a leading minus sign (45).
    """
    if not chars:
        raise ValueError("empty input")

    negative = False
    start = 0
    if chars[0] == 45:
        negative = True
        start = 1

    if start == len(chars):
        raise ValueError("no digits after minus sign")

    value = 0
    for code in chars[start:]:
        if not (48 <= code <= 57):
            raise ValueError(f"invalid character code: {code}")
        value = value * 10 + (code - 48)

    return -value if negative else value
