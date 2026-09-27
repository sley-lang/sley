# Tests

A test is a TestCase entity that targets a function:

    {"name": "t_small", "fn": "percent", "args": [1, 4], "expect": {"Ok": 25}}
    {"fn": "percent", "args": [1, 0], "expect": {"Err": "ZeroWhole"}}
    {"fn": "percent", "args": [1, 2], "expect": {"Ok": 50}, "limits": {"fuel": 5000}}
    {"fn": "stub", "args": [], "expect": {"trap": "unreachable"}}

`percent` is the guide's example; `stub` is a function whose body is still
`trap unreachable`. `name` defaults to `t_<fn>_<n>`. `expect` is read against the function's
result type (`sley-agent help types`). The limits default to the smaller of
the workbench defaults and the policy grant (fuel 1000000, memory 16 MiB,
output 64 KiB). The kernel checks the limits of the tests a candidate
selects (those that target a function it changes): a declared limit above
the grant is refused with CANDIDATE_TEST_RESOURCE_LIMIT, naming the limit.

`try` runs the tests the candidate touches. `sley-agent test c1` runs every
TestCase in the candidate's state, and `--public cases.json` also runs
`[{"name", "function", "args", "expect"}]` cases. Results are advisory: they
are never admission evidence and never committed.

Committing a candidate that changes a function together with tests that
target it needs native test evidence, which is not available in 2.0. Submit
such candidates instead, or commit the code first and the tests after it.
