Sley Python dialect: Python compiled to Sley; int is 64-bit and overflow traps; no floats.
Write typed Python (def f(xs: list[int]) -> int:); `sley-agent try --familiar` compiles it.
Types: int, bool, str, list[T], records. // and % floor as in Python; / is refused.
Has: if/elif/else, for over lists, range, enumerate, zip and slices, while, return,
= += -= *= //= %=, assert, raise (traps), in, chained comparisons, and/or (short-circuit),
truthiness, x if c else y, len min max abs ** math.gcd, sum any all max min over
generators and comprehensions, helper defs (no captured variables).
Not supported: break, continue, list values (append, slicing into a variable), dicts,
sets, tuples, classes, methods, strings as sequences.
