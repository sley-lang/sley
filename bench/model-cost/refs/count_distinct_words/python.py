import re


def count_distinct_words(s: str) -> int:
    return len({w.lower() for w in re.findall(r"[A-Za-z]+", s)})
