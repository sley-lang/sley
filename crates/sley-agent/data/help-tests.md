# Tests

A test is a TestCase entity that targets a function:

    {"name": "t_small", "fn": "area", "args": [3, 4], "expect": {"Ok": 12}}
    {"fn": "area", "args": [0, 4], "expect": {"Err": "BadSide"}}
    {"fn": "area", "args": [2, 5], "expect": {"Ok": 10}, "limits": {"fuel": 5000}}
    {"fn": "stub", "args": [], "expect": {"trap": "unreachable"}}

`area` is the guide's example; `stub` is a function whose body is still
`trap unreachable`. `name` defaults to `t_<fn>_<n>`. `expect` is read against
the function's result type (`sley-agent help types`). `limits` is an object
of integers with exactly these keys: `fuel`, `memory_bytes`,
`output_bytes`, `effect_count`, `call_depth`, `wall_timeout_millis`; any
other key is refused. A limit not given takes the smaller of the workbench
default and the policy grant (fuel 1000000, memory_bytes 16777216,
output_bytes 65536, effect_count 0; call_depth 256 and
wall_timeout_millis 10000). The kernel checks the limits of the tests a
candidate selects (those that target a function it changes): a declared
limit above the grant is refused with CANDIDATE_TEST_RESOURCE_LIMIT, naming
the limit.

In an AF1-X frame (`"afx": 1`), `test_tables` states a target and its
defaults once, then one row per case (`sley-agent help afx`). Each row
becomes one TestCase under the same rules.

`try` runs the tests its candidate touches and prints failures in full
(`--verbose` also lists passing tests). `sley-agent test c1` runs every
TestCase in the candidate's state, and `--public cases.json` also runs
`[{"name", "function", "args", "expect"}]` cases. `sley-agent import
cases.json --on d1` adds such cases to a draft as TestCases, recording the
file's digest; `try` counts tests as provided (already in the program),
imported, and authored (yours). Results are advisory: they are never
admission evidence and never committed.

Committing a candidate that changes a function together with tests that
target it needs native test evidence, which is not available in 2.0. Submit
such candidates instead, or commit the code first and the tests after it.
