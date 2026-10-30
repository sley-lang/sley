"""Task set v2 of the genuine model cost benchmark (2.2 measurement, item 0).

The 24 task statements and their examples are read from
`taskset-v2/authored.json`, exactly as the independent author wrote them
(see `taskset-v2/README.md`). This module adds what the author did not write:
the reference oracle and 128 seeded hidden cases per task (seed 218, edge
cases first). `tasks.py` (v1) is unchanged, so v1 runs stay reproducible.

Values: an int, a list of ints, a str (printable ASCII), or the string
"error" for the task's error. `show` gives the one-line form every language's
oracle driver prints: ints in decimal, lists and strings as compact JSON.

Not yet pre-registered: the held-out tasks are not sealed and no hash is in
`prereg.json` (Magister reviews the statements first).
"""
import json, math, os, random, re
from collections import Counter

HERE = os.path.dirname(os.path.abspath(__file__))
AUTHORED = os.path.join(HERE, "taskset-v2", "authored.json")

# Dated corrections to the authored statements or examples. Empty: every
# authored example agrees with the oracle (checked by `check_visible`).
AMENDMENTS = {}


class Error(Exception):
    """The reference's error result."""


def _load():
    out = []
    for t in json.load(open(AUTHORED)):
        t = dict(t, **AMENDMENTS.get(t["name"], {}))
        out.append({
            "name": t["name"],
            "group": "v2",
            "params": [tuple(p) for p in t["params"]],
            "returns": t["returns"],
            "errors": t["errors"],
            "statement": t["statement"],
            "visible": [(e["args"], e["expect"]) for e in t["examples"]],
        })
    return out


TASKS = _load()
BY_NAME = {t["name"]: t for t in TASKS}
TIERS = {"full": [t["name"] for t in TASKS]}


def show(value):
    """The canonical one-line form of a result (what the oracle drivers print)."""
    if value == "error":
        return "ERR"
    if isinstance(value, bool):
        raise TypeError("no bool results in v2")
    if isinstance(value, int):
        return str(value)
    return json.dumps(value, separators=(",", ":"))


# ---- the reference oracle ----------------------------------------------------

DIGITS = "0123456789"
LOWER = "abcdefghijklmnopqrstuvwxyz"
UPPER = LOWER.upper()


def _is_letter(c):
    return c in LOWER or c in UPPER


def _parse_signed_int(s):
    if not re.fullmatch(r"[+-]?[0-9]+", s):
        raise Error
    v = int(s)
    if v < -1000000 or v > 1000000:
        raise Error
    return v


def _rle_encode(s):
    out, i = [], 0
    while i < len(s):
        j = i
        while j < len(s) and s[j] == s[i]:
            j += 1
        out.append(f"{s[i]}{j - i}")
        i = j
    return "".join(out)


def _rle_decode(s):
    out, total, i = [], 0, 0
    while i < len(s):
        c = s[i]
        if c in DIGITS:
            raise Error
        i += 1
        j = i
        while j < len(s) and s[j] in DIGITS:
            j += 1
        if j == i or s[i] == "0":
            raise Error
        n = int(s[i:j])
        total += n
        if total > 1000:
            raise Error
        out.append(c * n)
        i = j
    return "".join(out)


def _top_k(xs, k):
    c = Counter(xs)
    return sorted(c, key=lambda v: (-c[v], v))[:k]


def _second_largest(xs):
    d = sorted(set(xs))
    if len(d) < 2:
        raise Error
    return d[-2]


def _chunk_sums(xs, size):
    if size < 1:
        raise Error
    return [sum(xs[i:i + size]) for i in range(0, len(xs), size)]


def _valid_username(s):
    ok = (3 <= len(s) <= 16 and s[0] in LOWER and all(c in LOWER or c in DIGITS or c == "_" for c in s)
          and "__" not in s and s[-1] != "_")
    return 1 if ok else 0


def _by_remainder(xs, m):
    if m < 1 or m > 200:
        raise Error
    out = [0] * m
    for x in xs:
        out[x % m] += 1
    return out


def _dense_rank(xs):
    order = {v: i + 1 for i, v in enumerate(sorted(set(xs), reverse=True))}
    return [order[x] for x in xs]


def _most_common_letter(s):
    c = Counter(ch.lower() for ch in s if _is_letter(ch))
    if not c:
        raise Error
    return min(c, key=lambda ch: (-c[ch], ch))


def _distinct_words(s):
    return len({w.lower() for w in re.findall(r"[A-Za-z]+", s)})


def _moving_average(xs, w):
    if w < 1:
        raise Error
    return [sum(xs[i:i + w]) // w for i in range(len(xs) - w + 1)]


def _covered(xs):
    if len(xs) % 2:
        raise Error
    pairs = [(xs[i], xs[i + 1]) for i in range(0, len(xs), 2)]
    if any(a > b for a, b in pairs):
        raise Error
    total, end = 0, None
    for a, b in sorted(pairs):
        if end is None or a > end:
            total += b - a
            end = b
        elif b > end:
            total += b - end
            end = b
    return total


def _longest_increasing(xs):
    best = run = 0
    for i, x in enumerate(xs):
        run = run + 1 if i and xs[i - 1] < x else 1
        best = max(best, run)
    return best


def _slugify(s):
    low = "".join(c.lower() if c in UPPER else c for c in s)
    return re.sub(r"[^a-z0-9]+", "-", low).strip("-")


def _clock(s):
    if len(s) != 8 or s[2] != ":" or s[5] != ":" or any(s[i] not in DIGITS for i in (0, 1, 3, 4, 6, 7)):
        raise Error
    h, m, sec = int(s[0:2]), int(s[3:5]), int(s[6:8])
    if h > 23 or m > 59 or sec > 59:
        raise Error
    return h * 3600 + m * 60 + sec


def _gcd_list(xs):
    g = 0
    for x in xs:
        g = math.gcd(g, abs(x))
    return g


ROMAN = [(1000, "M"), (900, "CM"), (500, "D"), (400, "CD"), (100, "C"), (90, "XC"), (50, "L"), (40, "XL"),
         (10, "X"), (9, "IX"), (5, "V"), (4, "IV"), (1, "I")]


def _roman(n):
    if n < 1 or n > 3999:
        raise Error
    out = []
    for v, sym in ROMAN:
        while n >= v:
            out.append(sym)
            n -= v
    return "".join(out)


def _caesar(s, k):
    k %= 26
    out = []
    for c in s:
        if c in LOWER:
            out.append(LOWER[(LOWER.index(c) + k) % 26])
        elif c in UPPER:
            out.append(UPPER[(UPPER.index(c) + k) % 26])
        else:
            out.append(c)
    return "".join(out)


def _brackets(s):
    pair = {")": "(", "]": "[", "}": "{"}
    stack, best = [], 0
    for c in s:
        if c in "([{":
            stack.append(c)
            best = max(best, len(stack))
        elif c in pair:
            if not stack or stack[-1] != pair[c]:
                raise Error
            stack.pop()
    if stack:
        raise Error
    return best


def _pairs(xs, target):
    return sum(1 for i in range(len(xs)) for j in range(i + 1, len(xs)) if xs[i] + xs[j] == target)


def _mask(s):
    total = sum(1 for c in s if c in DIGITS)
    if total <= 4:
        return s
    out, seen = [], 0
    for c in s:
        if c in DIGITS:
            seen += 1
            out.append("*" if seen <= total - 4 else c)
        else:
            out.append(c)
    return "".join(out)


def _overdraft(start, xs):
    bal = start
    for i, x in enumerate(xs):
        bal += x
        if bal < 0:
            return i
    return -1


REFERENCE = {
    "parse_signed_int": _parse_signed_int,
    "run_length_encode": _rle_encode,
    "run_length_decode": _rle_decode,
    "top_k_frequent": _top_k,
    "second_largest_distinct": _second_largest,
    "format_thousands": lambda n: f"{n:,}",
    "chunk_sums": _chunk_sums,
    "validate_username": _valid_username,
    "count_by_remainder": _by_remainder,
    "dense_rank": _dense_rank,
    "most_common_letter": _most_common_letter,
    "count_distinct_words": _distinct_words,
    "moving_average_floor": _moving_average,
    "covered_length": _covered,
    "longest_increasing_run": _longest_increasing,
    "slugify": _slugify,
    "parse_clock_seconds": _clock,
    "gcd_of_list": _gcd_list,
    "to_roman": _roman,
    "caesar_shift": _caesar,
    "bracket_depth": _brackets,
    "count_pairs_with_sum": _pairs,
    "mask_digits": _mask,
    "first_overdraft": _overdraft,
}


def reference(name, args):
    """The oracle's answer: an int, a list, a str, or Error."""
    return REFERENCE[name](*args)


# ---- hidden cases ------------------------------------------------------------

PRINTABLE = "".join(chr(c) for c in range(32, 127))
PUNCT = " !\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~"


def _ints(rng, n, lo, hi):
    return [rng.randint(lo, hi) for _ in range(n)]


def _list(rng, i, small, values=None):
    """A list of ints: the empty and tiny lists first, then mixed lengths up to 200."""
    n = i if small else rng.choice([rng.randint(0, 12), rng.randint(0, 40), rng.randint(150, 200)])
    span = values or rng.choice([2, 5, 50, 1000000])
    return _ints(rng, n, -span, span)


def _text(rng, n, alphabet=PRINTABLE):
    return "".join(rng.choice(alphabet) for _ in range(n))


def _length(rng, small, i):
    return i if small else rng.choice([rng.randint(0, 12), rng.randint(0, 40), rng.randint(150, 200)])


def _gen(name, rng, i):
    """The i-th generated argument list (i < 8 favors edge cases)."""
    small = i < 8
    if name == "parse_signed_int":
        fixed = ["", "+", "-", "0", "-0", "+1000000", "-1000001", "1000001"]
        if small:
            return [fixed[i]]
        kind = i % 5
        body = str(rng.randint(0, 10 ** rng.randint(1, 8)))
        if kind == 0:
            body = "0" * rng.randint(1, 4) + body
        sign = rng.choice(["", "", "+", "-"])
        s = sign + body
        if kind == 1:
            pos = rng.randrange(len(s) + 1)
            s = s[:pos] + rng.choice(" +-.a,x") + s[pos:]
        if kind == 2:
            s = rng.choice(["++", "--", "+-", " "]) + body
        return [s]
    if name == "run_length_encode":
        alphabet = "".join(c for c in PRINTABLE if c not in DIGITS)
        if small:
            return [["", "a", "aa", "ab", "  !!", "aaaaaaaaaaaa", "xyz", "abba"][i]]
        letters = rng.choice([alphabet, "ab", "aab ", "xyz!"])
        out = []
        while len(out) < _length(rng, False, i):
            out += [rng.choice(letters)] * rng.choice([1, 1, 2, 3, 12])
        return ["".join(out[:200])]
    if name == "run_length_decode":
        if small:
            return [["", "a1", "a3b1c2", "x10", "ab2", "a05", "3a", "a0"][i]]
        nondigit = "".join(c for c in PRINTABLE if c not in DIGITS)
        groups = [rng.choice(nondigit) + str(rng.choice([1, 2, 3, 9, 10, 12, 40, 250, 600])) for _ in range(rng.randint(0, 8))]
        s = "".join(groups)
        kind = i % 6
        if kind == 1 and s:
            pos = rng.randrange(len(s))
            s = s[:pos] + s[pos + 1:]
        if kind == 2:
            s = s + rng.choice(["a", "0", "b05", "1"])
        if kind == 3:
            s = rng.choice(nondigit) + "0" + str(rng.randint(1, 9)) + s
        return [s[:200]]
    if name == "top_k_frequent":
        xs = _list(rng, i, small, values=rng.choice([3, 10, 1000000]))
        return [xs, i % 4 if small else rng.choice([0, 1, 2, 3, 5, 10, 200])]
    if name == "second_largest_distinct":
        return [_list(rng, i, small, values=rng.choice([1, 3, 50, 1000000]))]
    if name == "format_thousands":
        fixed = [0, 1, -1, 999, 1000, -1000, 1000000, -1000000]
        if small:
            return [fixed[i]]
        return [rng.randint(-10 ** rng.randint(1, 6), 10 ** rng.randint(1, 6))]
    if name in ("chunk_sums", "moving_average_floor"):
        xs = _list(rng, i, small)
        size = (i % 4) - 1 if small else rng.choice([-1000000, -3, 0, 1, 2, 3, 7, 50, 199, 200, 201, 1000000])
        return [xs, size]
    if name == "validate_username":
        if small:
            return [["", "ab", "abc", "alice_99", "9lives", "bad__name", "trail_", "a" * 16][i]]
        kind = i % 4
        alphabet = LOWER + DIGITS + "_" if kind else PRINTABLE
        s = rng.choice(LOWER) + _text(rng, rng.randint(0, 18), alphabet)
        if kind == 3:
            s = rng.choice(LOWER + LOWER.upper() + DIGITS + "_") + s[1:]
        return [s]
    if name == "count_by_remainder":
        xs = _list(rng, i, small)
        m = [1, 2, 3, 0, -1, 200, 201, 7][i] if small else rng.choice([-5, 0, 1, 2, 3, 7, 10, 199, 200, 201, 1000000])
        return [xs, m]
    if name == "dense_rank":
        return [_list(rng, i, small, values=rng.choice([1, 3, 50, 1000000]))]
    if name in ("most_common_letter", "count_distinct_words", "slugify", "mask_digits"):
        if small:
            fixed = {
                "most_common_letter": ["", "a", "aAbB", "Hello World", "123 !!", "zzY y", "Z", "bbaa"],
                "count_distinct_words": ["", "a", "The cat and the hat", "Hi, hi! HI?", "don't stop", "a1b2c", "123", "A a"],
                "slugify": ["", "!!!", "Hello, World!", "C++ & Go", "--a--", "v2.0 Release", "ABC", "  x  "],
                "mask_digits": ["", "1234", "12345", "1234-5678-9012", "a1b2c3d4e5", "phone: 555 123 4567", "abc", "0"],
            }[name]
            return [fixed[i]]
        alphabet = rng.choice([PRINTABLE, LOWER + UPPER + " ", LOWER + " ,.!'-", DIGITS + "- ", LOWER + DIGITS + PUNCT])
        return [_text(rng, _length(rng, False, i), alphabet)]
    if name == "covered_length":
        if small:
            return [[[], [1, 2, 3], [5, 3], [1, 1], [0, 2, 2, 4], [1, 5, 3, 8], [-3, 0, 0, 3], [0, 10, 2, 3]][i]]
        n = rng.choice([1, 2, 5, 20, 100])
        span = rng.choice([10, 100, 1000000])
        xs = []
        for _ in range(n):
            a = rng.randint(-span, span)
            b = min(span, a + rng.randint(0, span // 2 + 1))
            if i % 7 == 0 and a > -span:
                a, b = b, a - 1
            xs += [a, b]
        if i % 11 == 0:
            xs.append(rng.randint(-span, span))
        return [xs[:200]]
    if name == "longest_increasing_run":
        return [_list(rng, i, small, values=rng.choice([2, 5, 1000000]))]
    if name == "parse_clock_seconds":
        fixed = ["00:00:00", "23:59:59", "24:00:00", "1:02:03", "12:60:00", "12:00:60", "", "12:34:56 "]
        if small:
            return [fixed[i]]
        h, m, s = rng.randint(0, 26), rng.randint(0, 62), rng.randint(0, 62)
        t = f"{h:02d}:{m:02d}:{s:02d}"
        kind = i % 5
        if kind == 1:
            pos = rng.randrange(8)
            t = t[:pos] + rng.choice("x.: 1") + t[pos + 1:]
        if kind == 2:
            t = rng.choice([t[1:], t + "0", t.replace(":", "-", 1)])
        return [t]
    if name == "gcd_of_list":
        if small:
            return [[[], [0], [0, 0], [7], [-4, 6], [0, 7], [12, 18, 24], [-1000000, 1000000]][i]]
        g = rng.choice([1, 2, 3, 6, 7, 12, 1000])
        n = rng.randint(1, 40)
        return [[rng.randint(-1000000 // g, 1000000 // g) * g for _ in range(n)]]
    if name == "to_roman":
        fixed = [0, 1, 4, 9, 3999, 4000, -1, 1994]
        if small:
            return [fixed[i]]
        return [rng.choice([rng.randint(1, 3999), rng.randint(1, 3999), rng.randint(-1000000, 1000000)])]
    if name == "caesar_shift":
        if small:
            return [["", "abc", "xyz", "Az", "Hello, World!", "abc", "Zz", "!"][i], [0, 1, 3, 52, -1, -27, 1000000, -1000000][i]]
        alphabet = rng.choice([PRINTABLE, LOWER + UPPER + " "])
        return [_text(rng, _length(rng, False, i), alphabet), rng.choice([rng.randint(-30, 30), rng.randint(-1000000, 1000000)])]
    if name == "bracket_depth":
        if small:
            return [["", "a", "()", "(]", "((", "a(b[c]{d})", "(()(()))", ")("][i]]
        n = _length(rng, False, i)
        if i % 2:
            # balanced: open and close in a random order of nesting
            out, stack = [], []
            while len(out) < n:
                if stack and (rng.random() < 0.45 or len(out) + len(stack) >= n):
                    out.append({"(": ")", "[": "]", "{": "}"}[stack.pop()])
                elif rng.random() < 0.2:
                    out.append(rng.choice("ab x1"))
                else:
                    c = rng.choice("([{")
                    stack.append(c)
                    out.append(c)
            out += [{"(": ")", "[": "]", "{": "}"}[c] for c in reversed(stack)]
            s = "".join(out)
            if i % 4 == 1 and s:
                pos = rng.randrange(len(s))
                s = s[:pos] + rng.choice("()[]{}") + s[pos + 1:]
            return [s[:200]]
        return [_text(rng, n, "()[]{}ab ")]
    if name == "count_pairs_with_sum":
        xs = _list(rng, i, small, values=rng.choice([3, 10, 1000000]))
        return [xs, rng.randint(-6, 6) if rng.random() < 0.8 else rng.randint(-1000000, 1000000)]
    if name == "first_overdraft":
        start = [0, -5, 10, 100, 0, -1000000, 1000000, 1][i] if small else rng.randint(-1000, 1000) * rng.choice([1, 1000])
        return [start, _list(rng, i, small, values=rng.choice([50, 1000, 1000000]))]
    raise KeyError(name)


def cases(name, count=128, seed=218):
    """(args, expected) pairs; expected is the reference value or "error"."""
    rng = random.Random(f"{seed}:v2:{name}")
    out = []
    for i in range(count):
        args = _gen(name, rng, i)
        try:
            want = reference(name, args)
        except Error:
            want = "error"
        out.append((args, want))
    return out


def in_bounds(name, args):
    """The arguments meet the statement's input bounds (lists of at most 200
    ints within +-10^6, printable-ASCII strings of at most 200 characters)."""
    for (_, kind), v in zip(BY_NAME[name]["params"], args):
        if kind == "int" and not -1000000 <= v <= 1000000:
            return False
        if kind == "list" and (len(v) > 200 or any(not -1000000 <= x <= 1000000 for x in v)):
            return False
        if kind == "str" and (len(v) > 200 or any(c not in PRINTABLE for c in v)):
            return False
    if name == "top_k_frequent" and not 0 <= args[1] <= 200:
        return False
    if name == "run_length_encode" and any(c in DIGITS for c in args[0]):
        return False
    return True


def check_visible():
    """The authored examples agree with the oracle. Returns the disagreements."""
    bad = []
    for t in TASKS:
        for args, want in t["visible"]:
            try:
                got = reference(t["name"], args)
            except Error:
                got = "error"
            if got != want:
                bad.append((t["name"], args, got, want))
    return bad
