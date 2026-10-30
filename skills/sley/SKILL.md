---
name: sley
description: Write, test and submit Sley programs with the `sley-agent` workbench, authoring in its typed Python dialect. Use when a task asks for a Sley function or program, mentions `sley-agent`, `try --familiar`, drafts such as `d1@r1`, or a Sley refusal symbol (`AGENT_FRAME_INVALID`, `CAP_BUDGET_EXCEEDED`, `TXN_TEST_EVIDENCE_UNSUPPORTED`, ...).
---

# Writing Sley programs

Sley is a programming system built for agents. A Sley program is a typed,
content-addressed semantic graph of functions, types, constants and tests,
not a source file. You change it by proposing a candidate; a deterministic
kernel validates the candidate in ordered phases and refuses anything it
cannot check, with a stable symbol. `sley-agent` is the workbench that builds
candidates, runs their tests and submits them.

You can write candidates in a typed Python dialect. It is an optional,
non-canonical authoring input: `try --familiar` parses the text once into a
structured frame, and from then on drafts, submissions and commits use the
frame and the graph, never the text. Nothing renders a program back into
Python. The JSON frame form (AF1) is in `sley-agent help`.

Every example below was run against the released 2.1.7 binaries.

## Install and verify

Linux x86_64 only.

```sh
curl -fsSLO https://github.com/sley-lang/sley/releases/download/v2.1.7/sley-2.1.7-linux-x86_64.tar.gz
curl -fsSLO https://github.com/sley-lang/sley/releases/download/v2.1.7/SHA256SUMS
sha256sum -c SHA256SUMS            # sley-2.1.7-linux-x86_64.tar.gz: OK
tar xzf sley-2.1.7-linux-x86_64.tar.gz
export PATH="$PWD/sley-2.1.7-linux-x86_64/bin:$PATH"
sley version                       # {"cli":"1","contract":"sley2-cli-v1","protocol_version":1}
sley-agent help python | head -1   # Sley Python dialect: Python compiled to Sley; int is 64-bit and overflow traps; no floats.
```

`sley-agent help python` is the dialect reference (about 25 lines). Read it
once per session. `sley-agent help` is the full guide; `help refusals` lists
every kernel symbol with its hint.

## The loop

Write the function and its tests in one file, `area.py`:

```python
def area(w: int, h: int) -> int:
    return w * h

def test_area():
    assert area(3, 4) == 12
    assert area(0, 9) == 0
    assert area(5, 5) == 25
```

```console
$ sley-agent init demo && cd demo
initialized demo (policy: fuel 1000000, memory 16777216, output 65536, 10000 mutations per candidate)
$ sley-agent try --familiar ../area.py
c1: 3 passed (advisory) d1@r1
$ sley-agent call area 3 4 --on c1
12
$ sley-agent --json call area 3 4 --on c1
{"fuel":6,"instructions":1,"result":12,"vm_micros":5}
$ sley-agent submit d1
submitted c1 from d1@r1 -> final_candidate.hex (2787-byte candidate, hex); resubmit any time, the last submission wins
```

- `c1` is a candidate handle and `d1@r1` is revision 1 of draft `d1`. Use the
  handles your own `try` prints. `latest` names the newest candidate.
- Fix a failure by editing the same file and running `try --familiar` again.
- `try --familiar tests.py --on d1` layers a file of only `test_*` functions
  onto draft `d1`, as revision `d1@r2`.
- `sley-agent test c1` runs every TestCase in the candidate's state;
  `status` shows the current submission and reruns its tests.
- `--verbose` on `try` prints the full report: changed entities, each test
  and the advisory policy. `sley-agent --json <command>` prints one JSON
  object; scripts should read that, not the text.
- `explain c1` repeats a verdict; `draft d1` shows a draft; `find` and
  `view <fn>` read the accepted state.

Exit status: `0` success, `1` a negative outcome (failing test, kernel
refusal, no submission), `2` a workbench refusal (text the frontend refused,
bad usage, unknown handle, refused submit or commit).

### Reading the one-line result

| Output | Meaning |
|---|---|
| `c1: 3 passed (advisory) d1@r1` | Valid candidate; every test it ran matched. |
| `FAIL test_bound_2 (bound): expected 10, got 0 (line 11)` then `c4: 1 failed, 2 passed (advisory) d5@r1` | Valid candidate, failing test; the line is in your file. |
| `c11: Valid (+9 created, 0 replaced, 0 deleted) draft d28@r1` with `tests: 0 ran (...)` | Valid, but nothing tested it. `submit` will refuse. |
| `../r.py:2:14: <message>`, source line, caret, `draft d9@r1: text refused` | The frontend refused the text (exit 2). No candidate. |
| same shape ending `draft d18@r1: incomplete` | The text parsed but the frame did not compile (type errors, unknown names). |
| `c10: REFUSED CapabilityDenied at phase 9 (capability/policy) ...` | The kernel refused the candidate; `symbol:`, `where:`, `hint:` follow. |

### `call`

Arguments are JSON, one per parameter: `3`, `-1`, `[5,6,7]`, `'"text"'`,
records as objects `'{"lo":2,"hi":9}'`. Results print the same way. A function
returning `Result` prints `{"Ok":12}` or `{"Err":"Empty"}`. A trap prints
`{"trap":1}` and running out of fuel prints `{"limit":"Fuel"}`, both with exit
status 0. `call` runs under fixed ceilings of 100,000,000 fuel and
instructions. `--reference` runs the reference interpreter instead of the
compact plan; result, fuel and instructions must match.

These calls use the functions of the dialect example below; `spin` is a
`while` loop that counts up until its fuel runs out.

```console
$ sley-agent call is_sorted '[1,3,2]' --on latest
false
$ sley-agent call width '{"lo":2,"hi":9}' --on latest
7
$ sley-agent call fib 93 --on latest
{"trap":1}
$ sley-agent call spin 1 --on latest
{"limit":"Fuel"}
```

With `--json`, `call` adds the exact `fuel` and `instructions` counts.

## The Python dialect

Every parameter and the result must be annotated. Types: `int` (64-bit,
overflow traps), `bool`, `str`, `list[T]`, and records.

```python
from dataclasses import dataclass
import pytest


@dataclass
class Span:
    lo: int
    hi: int


def width(s: Span) -> int:
    assert s.lo <= s.hi
    return s.hi - s.lo


def is_sorted(xs: list[int]) -> bool:
    return all(xs[i] <= xs[i + 1] for i in range(len(xs) - 1))


def first_neg(xs: list[int]) -> int:
    for i, x in enumerate(xs):
        if x < 0:
            return i
    return -1


def fib(n: int) -> int:
    a, b = 0, 1
    while n > 0:
        a, b = b, a + b
        n -= 1
    return a


def test_all():
    assert width(Span(2, 7)) == 5
    with pytest.raises(AssertionError):
        width(Span(5, 1))
    assert is_sorted([1, 2, 2, 9])
    assert not is_sorted([3, 1])
    assert first_neg([4, -1, -2]) == 1
    assert fib(10) == 55
```

Supported: `if`/`elif`/`else`, `while`, `for x in xs`, `range` (with step),
`enumerate`, `zip`, `for x in xs[a:b]`, `break`, `continue`, tuple assignment
(`a, b = b, a % b`), `divmod`, `+= -= *= //= %= <<= >>=`, `//` and `%` with
Python's flooring, `**`, `pow(b, e, m)`, `math.gcd`, `<< >>`, `&` with a
`2**k - 1` mask, chained comparisons, `in`, short-circuit `and`/`or`,
truthiness, `x if c else y`, `len min max abs`, `sum any all max min` over
generators and comprehensions, `len(set(xs))`, `assert` and `raise` (both
trap), and helper `def`s that capture no variables.

Records: a `@dataclass` class of annotated fields declares a record type.
Functions can take records and read their fields. In 2.1.7 a function body
cannot construct one (`Span(...)` there is refused); build records only as
test arguments or expected values, positional or keyword.

Refused at the line and column: floats and `/`, `None` and `is`, list values
(literals in a body, `append`, a stored or returned slice), dicts, sets,
tuple values and `tuple[...]` types, methods (`s.upper()`), `lambda`,
`try`/`with` in a function body, iterating a string, and function names that
are built-ins (`clamp`, `min`, ...). A Python `Enum` class is skipped without
creating a type; use the Rust-like syntax below for enums.

### Inline tests

A `def test_*():` holds only these lines, each becoming one TestCase:

- `assert f(args) == value`
- `assert f(args)` and `assert not f(args)` (truthiness)
- `with pytest.raises(E):` followed by `f(args)`

Arguments and expected values must be literals or record constructions,
not calls or arithmetic.
`pytest.raises(E)` is checked statically: `f`, or something it calls, must be
able to raise `E` (`raise E`, `//` or `%` for `ZeroDivisionError`, indexing
for `IndexError`, `assert` for `AssertionError`). At run time any trap
satisfies it.

### Rust-like familiar syntax: `struct` and `enum`

The same switch takes a second, Rust-like syntax (`help card`), which can
declare enums and return `Result`:

```text
struct Box { w: i64, h: i64 }
enum ShapeError { Empty, TooBig }

fn box_area(b: Box) -> Result<i64,ShapeError> {
    if b.w == 0 or b.h == 0 { return err(Empty) }
    if b.w > 1000 { return err(TooBig) }
    return ok(b.w * b.h)
}
```

```console
$ sley-agent try --familiar ../shape.txt
c13: Valid (+36 created, 0 replaced, 0 deleted) draft d29@r1
$ sley-agent try --on d29 '{"af1": 1, "afx": 1, "tests": [{"fn": "box_area", "args": [{"w": 3, "h": 4}], "expect": {"Ok": 12}}, {"fn": "box_area", "args": [{"w": 0, "h": 4}], "expect": {"Err": "Empty"}}]}'
c15: 2 passed (advisory) d29@r2
```

Add tests to it as an AF1 `tests` frame layered with `--on`, as shown.

## Reading refusals

Frontend and frame refusals point at your text, `file:line:col` with a caret.
The symbol is `AGENT_FRAME_INVALID`; `--verbose` lists every problem and
`--json` gives each obligation's `line` and `column`:

```console
$ sley-agent try --familiar ../r.py
../r.py:2:5: the function returns text, not i64
    return x
    ^
draft d18@r1: incomplete
```

Kernel refusals name the decision, the phase, the symbol and its numeric code,
where it failed and a hint:

```console
$ sley-agent try --familiar ../big.py
c10: REFUSED CapabilityDenied at phase 9 (capability/policy) (+31756 created, 0 replaced, 0 deleted) draft d27@r1
  symbol: CAP_BUDGET_EXCEEDED (38014)
  where: .max_mutation_count 31756 > grant ceiling 10000
  hint: The candidate has more operations than the policy's max_mutation_count. Split the change, or use a workspace whose policy grants more.
```

Symbols are stable, so match on them, not on message text. Kernel symbols
(`CFG_*`, `TYPE_*`, `TEST_PLAN_*`, `CANDIDATE_*`, `CAP_*`, ...) are listed
with hints in `sley-agent help refusals`. Workbench symbols you will meet:

| Symbol | Cause |
|---|---|
| `AGENT_FRAME_INVALID` | The text or frame is malformed or does not type-check. |
| `AGENT_USAGE_INVALID` | Unknown command or flag. Subcommands take no `--help`, and there is no `--`. |
| `AGENT_HANDLE_UNKNOWN` | No such candidate handle. |
| `AGENT_NAME_UNKNOWN` | `call` names a function the state lacks. |
| `AGENT_INPUT_INVALID` | A `call` argument does not fit the parameter type. |
| `AGENT_EXECUTION_REFUSED` | The candidate cannot run, for example because it is no longer Valid. |
| `AGENT_SUBMISSION_REFUSED` | `submit` of an untested or invalid candidate, or a refused `commit`. |
| `AGENT_DRAFT_STALE`, `AGENT_DRAFT_HEAD_CHANGED` | A draft revision is not the latest, or the head moved (`--rebase`). |

Runtime outcomes are not refusals. A trap reports as `trap(1)` in a test and
`{"trap":1}` from `call`. Code 1 is `unreachable`, which covers overflow, an
index out of range, `//` by zero, a failed `assert` and `raise`. Codes 2 to 4
are `resource_exhausted`, `adapter_contract_violation` and
`internal_invariant`.

Limits worth knowing: a test runs with 1,000,000 fuel by default; `call` has
100,000,000; a candidate may hold 10,000 mutations (each created entity is
one, and a function of a few dozen lines can create thousands); one expanded
function may hold 4,096 operations; text may nest 124 levels deep.

## Common mistakes

| Symptom | Fix |
|---|---|
| `/ is float division; Sley ints divide with //` | Use `//`. |
| `` `clamp` is a built-in; name the function otherwise `` | Rename the function. |
| `list values are not supported: loop, or use sum/any/all/len over a generator` | Fold over the input instead of building a list. |
| `` `tuple` is not supported `` | Return one value per function. |
| `` `is` is not supported: compare values with == `` | No `None`: use a sentinel such as `-1`. |
| `annotate the parameter: x: int` | Annotate every parameter and the result. |
| `` `Span` is not a known function with a complete signature `` | A body cannot construct a record in 2.1.7; take it as a parameter. |
| `a test argument is a literal or a record class declared above` | Pass literals in tests; test the inner function on its own. |
| `expected the end of the line, found` after `==` in a test | Write the expected value as a literal (`== 21`, not `== 81 % 7 + 1 + 12 + 4`). |
| `` `inc` cannot raise ValueError `` | Name an exception the function can raise, or add the `raise`. |
| `expected 7, got trap(1)` for `nth(xs, -1)` | A computed negative index traps. Literal `xs[-1]` works, as does `xs[len(xs) - 1]`. |
| `got trap(1)` on large inputs | Checked 64-bit overflow. Bound the input or the arithmetic. |
| `the expanded function has at least 7800 operations, more than the bound of 4096` | Split it into several functions. |
| `CAP_BUDGET_EXCEEDED` | Split the change across candidates. |
| `CANDIDATE_BASE_TRANSACTION_MISMATCH` from an old handle | The head moved after a commit; run `try` again. |
| `AGENT_SUBMISSION_REFUSED: c11 changes functions but no TestCase in it targets them` | Add tests (`try --on`), or `submit --untested` on purpose. |

## What the results mean

Keep four outcomes apart. `Valid` means the candidate passed static
validation; it moves no state. `3 passed (advisory)` means those test cases
matched their expected values under the advisory runner, which applies the
declared fuel and fixed VM caps but not the other declared limits. It is not
admission evidence, and passing tests do not show the program is correct for
other inputs. `submitted` writes `final_candidate.hex` and commits nothing.
`committed c11 as transaction 97ebb313` means the transaction engine accepted
it.

A candidate that changes code together with tests targeting it cannot be
committed in 2.1.7:

```console
$ sley-agent commit c1
error AGENT_SUBMISSION_REFUSED: commit refused: TXN_TEST_EVIDENCE_UNSUPPORTED
```

Native test evidence is not available yet, so submit such candidates, or
commit the code and then the tests in a later candidate. Dropping tests to
get a commit through records no tested behavior. See
[GUARANTEES.md](../../docs/GUARANTEES.md) for the full matrix and
[SLEY_AGENT_V1](../../docs/spec/SLEY_AGENT_V1.md) for the workbench contract.
