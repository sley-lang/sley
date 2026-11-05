def most_common_letter(s: str) -> str:
    counts: list[int] = [0] * 26
    for ch in s:
        c = ch
        if "A" <= c <= "Z":
            c = chr(ord(c) + 32)
        if "a" <= c <= "z":
            counts[ord(c) - 97] += 1
    best = 0
    for i in range(1, 26):
        if counts[i] > counts[best]:
            best = i
    if counts[best] == 0:
        raise ValueError("no ASCII letters")
    return chr(best + 97)
