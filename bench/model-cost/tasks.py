"""Task set v1 of the genuine model cost benchmark (pre-registered for 2.1.8).

Every task is one function over integers and lists of integers that returns an
integer, or signals an error. 18 tasks: 8 from the token-loop benchmark, 5 new
and 5 held out (2 of them edits to an existing program). The small tier drops
fib, collatz and isqrt. The oracle (`reference`, `cases`) is independent of
any model's tests: 128 seeded cases per task, edge cases first.
"""
import random

# params: (name, "int" | "list"); errors: whether some inputs are errors.
TASKS = [
    {"name": "fib", "group": "existing", "params": [("n", "int")], "errors": False,
     "statement": "Return the n-th Fibonacci number, with fib(0) = 0 and fib(1) = 1. 0 <= n <= 22.",
     "visible": [([0], 0), ([1], 1), ([10], 55), ([20], 6765)]},
    {"name": "collatz", "group": "existing", "params": [("limit", "int")], "errors": False,
     "statement": "Among the starting values 1 <= n < limit, return the one whose Collatz sequence takes the most steps to reach 1 (n -> n / 2 if n is even, else 3n + 1). On a tie, return the smallest such n. Return 0 when limit <= 1.",
     "visible": [([10], 9), ([100], 97), ([1], 0)]},
    {"name": "primes", "group": "existing", "params": [("n", "int")], "errors": False,
     "statement": "Return how many prime numbers are smaller than n. 0 <= n <= 2000.",
     "visible": [([10], 4), ([100], 25), ([0], 0)]},
    {"name": "digitsum", "group": "existing", "params": [("n", "int")], "errors": False,
     "statement": "Return the total of the decimal digits of every integer i with 0 <= i < n. 0 <= n <= 3000.",
     "visible": [([10], 45), ([100], 900), ([0], 0)]},
    {"name": "isqrt", "group": "existing", "params": [("n", "int")], "errors": False,
     "statement": "Return the sum, over every integer i with 0 <= i < n, of the integer square root of i (the largest r with r * r <= i). 0 <= n <= 2000.",
     "visible": [([10], 16), ([100], 615), ([0], 0)]},
    {"name": "inversions", "group": "existing", "params": [("xs", "list")], "errors": False,
     "statement": "Return the number of index pairs i < j with xs[i] > xs[j].",
     "visible": [([[3, 1, 2]], 2), ([[2, 2, 1]], 2), ([[5, 4, 3, 2, 1]], 10), ([[]], 0)]},
    {"name": "transform", "group": "existing", "params": [("xs", "list"), ("lo", "int"), ("hi", "int")], "errors": False,
     "statement": "Map every element x of xs to 2x if x is odd and to x if it is even, clamp the result into [lo, hi], and return the sum. lo <= hi.",
     "visible": [([[1, 2, 3, -4], 0, 5], 9), ([[], 0, 1], 0), ([[7], -3, 3], 3)]},
    {"name": "parse_int", "group": "existing", "params": [("chars", "list")], "errors": True,
     "statement": "chars holds character codes. Parse them as a decimal integer with an optional leading minus sign (code 45); digits are codes 48 to 57. It is an error if chars is empty, is only a minus sign, or contains any other code.",
     "visible": [([[49, 50, 51]], 123), ([[45, 55]], -7), ([[]], "error"), ([[45]], "error"), ([[49, 120]], "error")]},
    {"name": "longest_run", "group": "new", "params": [("xs", "list")], "errors": False,
     "statement": "Return the length of the longest run of equal adjacent elements of xs (0 for an empty list).",
     "visible": [([[1, 1, 2, 2, 2, 1]], 3), ([[]], 0), ([[5]], 1)]},
    {"name": "second_largest", "group": "new", "params": [("xs", "list")], "errors": True,
     "statement": "Return the second largest distinct value in xs. It is an error if xs has fewer than two distinct values.",
     "visible": [([[3, 9, 4, 9]], 4), ([[-1, -5]], -5), ([[7, 7]], "error"), ([[]], "error")]},
    {"name": "pair_count", "group": "new", "params": [("xs", "list"), ("k", "int")], "errors": False,
     "statement": "Return the number of index pairs i < j with xs[i] + xs[j] == k.",
     "visible": [([[1, 2, 3, 4], 5], 2), ([[2, 2, 2], 4], 3), ([[], 0], 0)]},
    {"name": "max_depth", "group": "new", "params": [("codes", "list")], "errors": True,
     "statement": "codes holds character codes. Return the maximum nesting depth of parentheses, where code 40 opens and code 41 closes; ignore every other code. It is an error if the parentheses are unbalanced (a close with no open before it, or an open never closed).",
     "visible": [([[40, 40, 41, 41]], 2), ([[97, 40, 98, 41, 40, 41]], 1), ([[]], 0), ([[41, 40]], "error"), ([[40]], "error")]},
    {"name": "mode_value", "group": "new", "params": [("xs", "list")], "errors": True,
     "statement": "Return the value that occurs most often in xs; on a tie, return the smallest such value. It is an error if xs is empty.",
     "visible": [([[3, 1, 3, 2, 1]], 1), ([[4, 4, -2]], 4), ([[]], "error")]},
    {"name": "max_subarray", "group": "heldout", "params": [("xs", "list")], "errors": True,
     "statement": "Return the largest sum of a non-empty contiguous slice of xs. It is an error if xs is empty.",
     "visible": [([[-2, 1, -3, 4, -1, 2, 1, -5, 4]], 6), ([[-3, -1]], -1), ([[]], "error")]},
    {"name": "count_leq", "group": "heldout", "params": [("xs", "list"), ("x", "int")], "errors": False,
     "statement": "xs is sorted in ascending order. Return how many elements of xs are less than or equal to x.",
     "visible": [([[1, 3, 3, 7], 3], 3), ([[1, 3, 3, 7], 0], 0), ([[], 5], 0)]},
    {"name": "root_sum", "group": "heldout", "params": [("n", "int")], "errors": False,
     "statement": "Return the sum of the digital roots of the integers 1 to n inclusive (the digital root repeats summing decimal digits until one digit is left). 0 <= n <= 3000.",
     "visible": [([9], 45), ([10], 46), ([0], 0)]},
    {"name": "clamp_sum", "group": "heldout", "params": [("xs", "list"), ("lo", "int"), ("hi", "int")], "errors": True, "edit": True,
     "statement": "Change the existing function: skip negative elements entirely (before clamping), and make lo > hi an error. Everything else stays as it is.",
     "visible": [([[5, -3, 12], 0, 10], 15), ([[-1, -2], 0, 5], 0), ([[1], 3, 2], "error")]},
    {"name": "count_multiples", "group": "heldout", "params": [("xs", "list"), ("k", "int")], "errors": True, "edit": True,
     "statement": "The existing function ignores k and counts even values. Fix it to count the elements divisible by k, and make k <= 0 an error.",
     "visible": [([[3, 6, 7, -9], 3], 3), ([[1, 2], 1], 2), ([[4], 0], "error")]},
]

SMALL_DROPS = {"fib", "collatz", "isqrt"}
TIERS = {
    "small": [t["name"] for t in TASKS if t["name"] not in SMALL_DROPS],
    "full": [t["name"] for t in TASKS],
}
BY_NAME = {t["name"]: t for t in TASKS}


class Error(Exception):
    """The reference's error result."""


def _digits(i):
    return sum(int(c) for c in str(i))


def _root(i):
    while i >= 10:
        i = _digits(i)
    return i


def reference(name, args):
    """The oracle's answer: an int, or Error."""
    if name == "fib":
        a, b = 0, 1
        for _ in range(args[0]):
            a, b = b, a + b
        return a
    if name == "collatz":
        best, best_n = -1, 0
        for n in range(1, args[0]):
            x, steps = n, 0
            while x != 1:
                x = x // 2 if x % 2 == 0 else 3 * x + 1
                steps += 1
            if steps > best:
                best, best_n = steps, n
        return best_n
    if name == "primes":
        return sum(1 for k in range(2, args[0]) if all(k % d for d in range(2, int(k ** 0.5) + 1)))
    if name == "digitsum":
        return sum(_digits(i) for i in range(args[0]))
    if name == "isqrt":
        import math
        return sum(math.isqrt(i) for i in range(args[0]))
    if name == "inversions":
        xs = args[0]
        return sum(1 for i in range(len(xs)) for j in range(i + 1, len(xs)) if xs[i] > xs[j])
    if name == "transform":
        xs, lo, hi = args
        return sum(min(max(2 * x if x % 2 else x, lo), hi) for x in xs)
    if name == "parse_int":
        c = args[0]
        if not c or c == [45]:
            raise Error
        sign, body = (-1, c[1:]) if c[0] == 45 else (1, c)
        if any(x < 48 or x > 57 for x in body):
            raise Error
        return sign * int("".join(chr(x) for x in body))
    if name == "longest_run":
        best = run = 0
        for i, x in enumerate(args[0]):
            run = run + 1 if i and args[0][i - 1] == x else 1
            best = max(best, run)
        return best
    if name == "second_largest":
        d = sorted(set(args[0]))
        if len(d) < 2:
            raise Error
        return d[-2]
    if name == "pair_count":
        xs, k = args
        return sum(1 for i in range(len(xs)) for j in range(i + 1, len(xs)) if xs[i] + xs[j] == k)
    if name == "max_depth":
        depth = best = 0
        for c in args[0]:
            if c == 40:
                depth += 1
                best = max(best, depth)
            elif c == 41:
                depth -= 1
                if depth < 0:
                    raise Error
        if depth:
            raise Error
        return best
    if name == "mode_value":
        xs = args[0]
        if not xs:
            raise Error
        return min(set(xs), key=lambda v: (-xs.count(v), v))
    if name == "max_subarray":
        xs = args[0]
        if not xs:
            raise Error
        best = cur = xs[0]
        for x in xs[1:]:
            cur = max(x, cur + x)
            best = max(best, cur)
        return best
    if name == "count_leq":
        return sum(1 for v in args[0] if v <= args[1])
    if name == "root_sum":
        return sum(_root(i) for i in range(1, args[0] + 1))
    if name == "clamp_sum":
        xs, lo, hi = args
        if lo > hi:
            raise Error
        return sum(min(max(x, lo), hi) for x in xs if x >= 0)
    if name == "count_multiples":
        xs, k = args
        if k <= 0:
            raise Error
        return sum(1 for x in xs if x % k == 0)
    raise KeyError(name)


def _ints(rng, n, lo, hi):
    return [rng.randint(lo, hi) for _ in range(n)]


def _gen(name, rng, i):
    """The i-th generated argument list (i < 8 favors edge cases)."""
    small = i < 8
    if name in ("fib",):
        return [i if small else rng.randint(0, 22)]
    if name == "collatz":
        return [i if small else rng.randint(0, 300)]
    if name in ("primes", "isqrt"):
        return [i if small else rng.randint(0, 2000)]
    if name in ("digitsum", "root_sum"):
        return [i if small else rng.randint(0, 3000)]
    if name in ("inversions", "longest_run", "mode_value", "max_subarray", "second_largest"):
        n = i if small else rng.randint(0, 40)
        span = rng.choice([2, 5, 50])
        return [_ints(rng, n, -span, span)]
    if name == "transform" or name == "clamp_sum":
        lo = rng.randint(-50, 100)
        hi = lo + rng.randint(-5 if name == "clamp_sum" else 0, 80)
        return [_ints(rng, i if small else rng.randint(0, 30), -100, 100), lo, hi]
    if name == "parse_int":
        kind = i % 6
        if kind == 0:
            return [[]] if i == 0 else [[45]]
        digits = [ord(c) for c in str(rng.randint(0, 10 ** rng.randint(1, 12)))]
        if kind == 1:
            return [[45] + digits]
        if kind == 2:
            pos = rng.randrange(len(digits) + 1)
            return [digits[:pos] + [rng.choice([32, 43, 46, 47, 58, 97])] + digits[pos:]]
        return [digits]
    if name == "pair_count":
        xs = _ints(rng, i if small else rng.randint(0, 40), -10, 10)
        return [xs, rng.randint(-12, 12)]
    if name == "max_depth":
        n = i if small else rng.randint(0, 40)
        codes = [rng.choice([40, 41, 40, 41, 97, 32]) for _ in range(n)]
        if rng.random() < 0.6:  # make many balanced
            depth, out = 0, []
            for c in codes:
                if c == 41 and depth == 0:
                    c = 40
                depth += 1 if c == 40 else -1 if c == 41 else 0
                out.append(c)
            codes = out + [41] * depth
        return [codes]
    if name == "count_leq":
        xs = sorted(_ints(rng, i if small else rng.randint(0, 40), -30, 30))
        return [xs, rng.randint(-35, 35)]
    if name == "count_multiples":
        return [_ints(rng, i if small else rng.randint(0, 30), -60, 60), rng.randint(-2, 12)]
    raise KeyError(name)


def cases(name, count=128, seed=218):
    """(args, expected) pairs; expected is an int or the string "error"."""
    rng = random.Random(f"{seed}:{name}")
    out = []
    for i in range(count):
        args = _gen(name, rng, i)
        try:
            want = reference(name, args)
        except Error:
            want = "error"
        out.append((args, want))
    return out


def check_visible():
    """The visible cases agree with the oracle (run by the unit tests)."""
    for t in TASKS:
        for args, want in t["visible"]:
            try:
                got = reference(t["name"], args)
            except Error:
                got = "error"
            assert got == want, (t["name"], args, got, want)
