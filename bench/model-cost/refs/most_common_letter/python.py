LETTERS = "abcdefghijklmnopqrstuvwxyz"


def most_common_letter(s: str) -> str:
    counts = {c: 0 for c in LETTERS}
    for ch in s:
        lower = ch.lower() if ch.isascii() else ch
        if lower in counts:
            counts[lower] += 1
    best = max(LETTERS, key=lambda c: (counts[c], -ord(c)))
    if counts[best] == 0:
        raise ValueError("no ASCII letters")
    return best
