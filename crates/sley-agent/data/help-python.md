Sley Python dialect: Python compiled to Sley; int is 64-bit and overflow traps; no floats.
Not supported: None, list values (append, slicing into a variable), dicts, sets (len(set(xs))
works), tuple values, methods, strings as sequences, classes other than records.
A class of annotated fields (`@dataclass`) declares a record type. A `def test_*():` of
`assert f(...) == value` lines and `with pytest.raises(E): f(...)` (any trap) adds TestCases.
Record values in tests require each field once; positional arguments precede keyword arguments.
`assert f(...)` and `assert not f(...)` use truthiness (bool, int, str, list),
including functions already in the workspace or the `--on` candidate.
Non-Boolean assertions add private predicate functions and ordinary Boolean TestCases.
Write typed Python (def f(xs: list[int]) -> int:); `sley-agent try --familiar` compiles it.
Types: int, bool, str, list[T], records. // and % floor as in Python; / is refused.
Has: if/elif/else, for over lists, range, enumerate, zip and slices, while, break, continue,
return, = += -= *= //= %= <<= >>=, a, b = b, a % b, divmod, assert, raise (traps), in,
chained comparisons, and/or (short-circuit), truthiness, x if c else y, len min max abs,
** pow(b, e, m) math.gcd, << >> and & 2**k-1 masks, sum any all max min over generators and
comprehensions, len(set(xs)), helper defs (no captured variables).
