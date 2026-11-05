def count_distinct_words(s: str) -> int:
    words: list[str] = []
    cur: str = ""
    for ch in s:
        if ("A" <= ch <= "Z") or ("a" <= ch <= "z"):
            if "A" <= ch <= "Z":
                cur += chr(ord(ch) + 32)
            else:
                cur += ch
        else:
            if cur != "":
                words.append(cur)
                cur = ""
    if cur != "":
        words.append(cur)
    total = 0
    for i in range(len(words)):
        is_first = True
        j = 0
        while j < i:
            if words[j] == words[i]:
                is_first = False
            j += 1
        if is_first:
            total += 1
    return total
