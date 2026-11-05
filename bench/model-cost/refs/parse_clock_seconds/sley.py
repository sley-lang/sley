def parse_clock_seconds(s: str) -> int:
    if len(s) != 8 or s[2] != ":" or s[5] != ":":
        raise ValueError("invalid format")
    parts: list[str] = [s[0:2], s[3:5], s[6:8]]
    values: list[int] = []
    for f in parts:
        value = 0
        for c in f:
            if not ("0" <= c <= "9"):
                raise ValueError("invalid digits")
            value = value * 10 + (ord(c) - 48)
        values.append(value)
    if values[0] > 23 or values[1] > 59 or values[2] > 59:
        raise ValueError("value out of range")
    return values[0] * 3600 + values[1] * 60 + values[2]
