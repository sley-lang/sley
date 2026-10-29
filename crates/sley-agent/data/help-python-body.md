Write typed Python (def f(xs: list[int]) -> int:); `sley-agent try --familiar` compiles it.
Only parameters and returns need types; locals are inferred (total = 0).
Types: int, bool, str, list[T], records. // and % floor as in Python; / is refused.
Has: if/elif/else, for over lists, range, enumerate, zip and slices, while, break, continue,
return, = += -= *= //= %= <<= >>=, a, b = b, a % b, divmod, assert, raise (traps), in,
chained comparisons, and/or (short-circuit), truthiness, x if c else y, len min max abs,
** pow(b, e, m) math.gcd, << >> and & 2**k-1 masks, sum any all max min over generators and
comprehensions, len(set(xs)), helper defs (no captured variables).
Instead of Counter or dicts: c = sum(1 for y in xs if y == x); bisect: lo, hi = 0, len(xs)
and while lo < hi; a [v] * n sieve: while d * d <= k for each k; clamping: min(max(v, lo), hi).
