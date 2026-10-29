A `def test_*():` of `assert f(...) == value` lines and `with pytest.raises(E): f(...)` adds
TestCases (f must be able to raise E: raise E or a subclass, // % for ZeroDivisionError,
indexing for IndexError).
The static class check follows built-in ancestry, OSError aliases, and declared subclasses.
Record values in tests require each field once; positional arguments precede keyword arguments.
`assert f(...)` and `assert not f(...)` use truthiness (bool, int, str, list),
including functions already in the workspace or the `--on` candidate.
Non-Boolean assertions add private predicate functions and ordinary Boolean TestCases.
