DIGITS = "0123456789"


def run_length_decode(s: str) -> str:
    parts = []
    total = 0
    i = 0
    while i < len(s):
        c = s[i]
        if c in DIGITS or not (32 <= ord(c) <= 126):
            raise ValueError("expected a non-digit character")
        i += 1
        j = i
        while j < len(s) and s[j] in DIGITS:
            j += 1
        if j == i or s[i] == "0":
            raise ValueError("invalid count")
        count = int(s[i:j])
        total += count
        if total > 1000:
            raise ValueError("decoded string too long")
        parts.append(c * count)
        i = j
    return "".join(parts)
