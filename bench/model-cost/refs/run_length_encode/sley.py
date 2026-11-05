def run_length_encode(s: str) -> str:
    parts: list[str] = []
    i = 0
    while i < len(s):
        j = i
        while j < len(s) and s[j] == s[i]:
            j += 1
        parts.append(s[i])
        parts.append(str(j - i))
        i = j
    return "".join(parts)
