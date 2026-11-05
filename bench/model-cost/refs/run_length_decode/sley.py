def run_length_decode(s: str) -> str:
    parts: list[str] = []
    total = 0
    i = 0
    while i < len(s):
        c = s[i]
        if "0" <= c <= "9" or not (32 <= ord(c) <= 126):
            raise ValueError("expected a non-digit character")
        i += 1
        j = i
        while j < len(s) and "0" <= s[j] <= "9":
            j += 1
        if j == i or s[i] == "0":
            raise ValueError("invalid count")
        count = 0
        k = i
        while k < j:
            count = count * 10 + (ord(s[k]) - ord("0"))
            k += 1
        total += count
        if total > 1000:
            raise ValueError("decoded string too long")
        k = 0
        while k < count:
            parts.append(c)
            k += 1
        i = j
    return "".join(parts)
