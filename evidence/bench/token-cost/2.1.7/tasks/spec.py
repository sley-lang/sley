"""Token benchmark spec: test cases and deliberate bugs, per task and form.

Forms: python (CPython, idiomatic typed Python + pytest), sley_py (Sley Python
dialect), sley_fam (Sley familiar syntax), sley_af1 (AF1-X structured JSON frame).
Every form implements the same function with the same test cases.
"""
ERR = object()  # an error case: Python raises ValueError; Sley returns Err(Case) or traps (Python dialect)

# (args, expected) ; expected may be ("err", Case)
CASES = {
    "fib": [([0], 0), ([1], 1), ([10], 55), ([20], 6765)],
    "loopsum": [([0], 0), ([10], 285), ([1000], 332833500)],
    "collatz": [([10], 9), ([100], 97)],
    "primes": [([10], 4), ([100], 25)],
    "gcdsum": [([5], 24), ([10], 145)],
    "parse_int": [([[49, 50, 51]], 123), ([[45, 55]], -7), ([[]], ("err", "Empty")),
                  ([[45]], ("err", "LoneMinus")), ([[49, 120]], ("err", "NotDigit"))],
    "transform": [([[1, 2, 3, -4], 0, 5], 9), ([[], 0, 1], 0)],
    "records": [([{"x": 1, "y": 2}, {"x": 4, "y": -2}], 7), ([{"x": 0, "y": 0}, {"x": 0, "y": 0}], 0)],
    "errors": [([100, 30], 70), ([50, 50], 0), ([100, 0], ("err", "NotPositive")),
               ([10, 30], ("err", "Insufficient"))],
    "tak": [([6, 4, 2], 3), ([12, 8, 4], 5), ([1, 2, 3], 3)],
    "triples": [([5], 1), ([20], 6)],
    "digitsum": [([10], 45), ([100], 900)],
    "isqrt": [([10], 16), ([100], 615)],
    "inversions": [([[3, 1, 2]], 2), ([[2, 2, 1]], 2), ([[5, 4, 3, 2, 1]], 10), ([[]], 0)],
}
TYPES = {"parse_int", "records", "errors"}  # tasks whose Sley text forms need a types frame first

# Deliberate bugs: (old, new) applied to the correct program; fixed by the reverse edit.
# "logic": wrong answer caught by tests. "typo": an undefined name.
BUGS = {
 "fib": {
  "logic": {"python": ("n < 2", "n < 1"), "sley_py": ("n < 2", "n < 1"), "sley_fam": ("n < 2", "n < 1"),
            "sley_af1": ('["lt", "n", 2]', '["lt", "n", 1]')},
  "typo": {"python": ("fib(n - 2)", "fib(m - 2)"), "sley_py": ("fib(n - 2)", "fib(m - 2)"),
           "sley_fam": ("fib(n - 2)", "fib(m - 2)"), "sley_af1": ('["sub", "n", 2]', '["sub", "m", 2]')}},
 "loopsum": {
  "logic": {f: ("i * i", "i + i") for f in ("python", "sley_py", "sley_fam")} | {"sley_af1": ('["mul", "i", "i"]', '["add", "i", "i"]')},
  "typo": {f: ("s + i * i", "s + j * i") for f in ("python", "sley_py", "sley_fam")} | {"sley_af1": ('["mul", "i", "i"]', '["mul", "j", "i"]')}},
 "collatz": {
  "logic": {f: ("best_n = n", "best_n = steps") for f in ("python", "sley_py", "sley_fam")} | {"sley_af1": ('["set", "best_n", "n"]', '["set", "best_n", "steps"]')},
  "typo": {f: ("steps > best", "steps > bets") for f in ("python", "sley_py", "sley_fam")} | {"sley_af1": ('["gt", "steps", "best"]', '["gt", "steps", "bets"]')}},
 "primes": {
  "logic": {f: ("d * d <= k", "d * d < k") for f in ("python", "sley_py", "sley_fam")} | {"sley_af1": ('["le", ["mul", "d", "d"], "k"]', '["lt", ["mul", "d", "d"], "k"]')},
  "typo": {f: ("k % d == 0", "k % b == 0") for f in ("python", "sley_py", "sley_fam")} | {"sley_af1": ('["rem", "k", "d"]', '["rem", "k", "b"]')}},
 "gcdsum": {
  "logic": {"python": ("for j in range(1, n)", "for j in range(0, n)"), "sley_py": ("for j in range(1, n)", "for j in range(0, n)"),
            "sley_fam": ("for j in 1..n", "for j in 0..n"), "sley_af1": ('["range", "j", 1, "n"', '["range", "j", 0, "n"')},
  "typo": {"python": ("s += gcd(i, j)", "s += gcd(i, k)"), "sley_py": ("s += gcd(i, j)", "s += gcd(i, k)"),
           "sley_fam": ("s += gcd(i, j)", "s += gcd(i, k)"), "sley_af1": ('["call", "gcd", "i", "j"]', '["call", "gcd", "i", "k"]')}},
 "parse_int": {
  "logic": {"python": ("(c - 48)", "(c - 47)"), "sley_py": ("(c - 48)", "(c - 47)"), "sley_fam": ("(c - 48)", "(c - 47)"),
            "sley_af1": ('["sub", "c", 48]', '["sub", "c", 47]')},
  "typo": {"python": ("value * 10", "valu * 10"), "sley_py": ("value * 10", "valu * 10"), "sley_fam": ("value * 10", "valu * 10"),
           "sley_af1": ('["mul", "value", 10]', '["mul", "valu", 10]')}},
 "transform": {
  "logic": {"python": ("x * 2", "x * 3"), "sley_py": ("x * 2", "x * 3"), "sley_fam": ("x * 2", "x * 3"),
            "sley_af1": ('["mul", "x", 2]', '["mul", "x", 3]')},
  "typo": {"python": ("lo), hi)", "lo), hl)"), "sley_py": ("lo), hi)", "lo), hl)"), "sley_fam": ("lo, hi)", "lo, hl)"),
           "sley_af1": ('"lo", "hi"]', '"lo", "hl"]')}},
 "records": {
  "logic": {f: ("a.y - b.y", "a.y - b.x") for f in ("python", "sley_py", "sley_fam")} | {"sley_af1": ('["field", "b", "y"]', '["field", "b", "x"]')},
  "typo": {f: ("a.x - b.x", "a.x - c.x") for f in ("python", "sley_py", "sley_fam")} | {"sley_af1": ('["sub", ["field", "a", "x"], ["field", "b", "x"]]', '["sub", ["field", "a", "x"], ["field", "c", "x"]]')}},
 "errors": {
  "logic": {f: ("amount > balance", "amount >= balance") for f in ("python", "sley_py", "sley_fam")} | {"sley_af1": ('["gt", "amount", "balance"]', '["ge", "amount", "balance"]')},
  "typo": {f: ("balance - amount", "balance - amout") for f in ("python", "sley_py", "sley_fam")} | {"sley_af1": ('["sub", "balance", "amount"]', '["sub", "balance", "amout"]')}},
 "tak": {
  "logic": {f: ("return z", "return y") for f in ("python", "sley_py")},
  "typo": {f: ("tak(z - 1, x, y)", "tak(z - 1, x, w)") for f in ("python", "sley_py")}},
 "triples": {
  "logic": {f: ("range(1, n + 1)", "range(1, n)") for f in ("python", "sley_py")},
  "typo": {f: ("count += 1", "cnt += 1") for f in ("python", "sley_py")}},
 "digitsum": {
  "logic": {f: ("x > 0", "x > 1") for f in ("python", "sley_py")},
  "typo": {f: ("x % 10", "z % 10") for f in ("python", "sley_py")}},
 "isqrt": {
  "logic": {f: ("mid * mid <= i", "mid * mid < i") for f in ("python", "sley_py")},
  "typo": {f: ("total += lo", "total += low") for f in ("python", "sley_py")}},
 "inversions": {
  "logic": {f: ("xs[j] < x", "xs[j] <= x") for f in ("python", "sley_py")},
  "typo": {f: ("count += 1", "cnt += 1") for f in ("python", "sley_py")}},
}
NEW = ["tak", "triples", "digitsum", "isqrt", "inversions"]

