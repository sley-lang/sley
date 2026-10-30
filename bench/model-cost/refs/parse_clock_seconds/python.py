DIGITS = "0123456789"


def parse_clock_seconds(s: str) -> int:
    if len(s) != 8 or s[2] != ":" or s[5] != ":":
        raise ValueError("invalid format")
    fields = (s[0:2], s[3:5], s[6:8])
    if any(c not in DIGITS for f in fields for c in f):
        raise ValueError("invalid digits")
    hh, mm, ss = (int(f) for f in fields)
    if hh > 23 or mm > 59 or ss > 59:
        raise ValueError("value out of range")
    return hh * 3600 + mm * 60 + ss
