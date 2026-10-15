# Sley 2.1.1 release notes

Sley 2.1.1 makes writing Sley about as cheap for an agent as writing the languages it already
knows. Agents can now author Sley in a typed **Python dialect**: Python syntax, compiled by
`sley-agent try --familiar` into the same structured bodies and the same Sley graph as every
other authoring path. The familiar syntax also becomes shorter and more forgiving. Nothing about
what a committed program means changes: the kernel, SSMC1, SCB1, the opcode set, admission and
commit are untouched.

The decision record is [ADR-0057](../adr/ADR-0057-python-dialect-and-authoring-parity.md), under
the optional-frontend policy of [ADR-0055](../adr/ADR-0055-familiar-authoring-frontend.md). The
contract is [SLEY_AGENT_V1](../spec/SLEY_AGENT_V1.md) sections 5.4, 5.5 and 5.5.1.

## Authoring cost

The maintainers measured authoring cost on a cross-language function benchmark. Eight hosted
coding models wrote the same functions in Sley and in Python, Go, Rust, C and C++. Each answer
was checked against an independent oracle on 128 cases, with up to three attempts using the
compiler's or the tests' feedback. Cost was counted per correct answer at the providers' list
prices. Sley was written in the Python dialect, with the first line of `help python` as its only
guidance.

- **Against 2.1.0.** On the same tasks and models, 2.1.1 cuts the cost of a correct Sley answer
  by about 40% on the development tasks and 53% on the validation tasks (medians over models). It
  also uses 53–63% fewer tokens.
- **Against other languages, development tasks.** A correct Sley answer costs 0.86x C, 1.01x C++,
  1.04x Rust, 1.16x Python and 1.27x Go (median over models, two full runs).
- **Ten held-out tasks** written after development. Sley costs 1.4–1.8x Python, Go and Rust, and
  95% of its answers are correct after feedback. With the first three lines of `help python` as
  guidance, the first attempt succeeds 93% of the time.

These are maintainers' measurements on short function tasks. They are not a claim about other
workloads, other models or editing existing programs.

## The Python dialect

Text whose first line is a `def` (or an `import`, `from`, `class` or decorator line) is the
Python dialect. It keeps Python's meaning wherever Sley can express it exactly, and refuses
everything else at its line and column:

- floor division and modulo (`//`, `%`, `divmod`), `**`, `pow(b, e, m)`, `math.gcd`, shifts and
  low-bit masks;
- truthiness, short-circuit `and` and `or`, chained comparisons, `in` and `not in`;
- `for` over lists, `range` (with a step), `enumerate`, `zip` and slices, plus `while`, `break`
  and `continue`;
- `sum`, `any`, `all`, `max`, `min` and `len` over lists, slices, generators and comprehensions
  (several `for` clauses, `default=`), and `len(set(xs))`;
- tuple assignment, Python's function-wide scoping for branches and loop variables, assignable
  parameters, and helper functions.

Python's single integer type is Sley's `i64`: an overflow traps, as all Sley arithmetic does.
Lengths and indexes are converted with explicit, checked operations. Floats, `None`, list values,
dicts, sets (other than `len(set(xs))`), tuple values and methods are refused. `help python` is
the short reference, and its first line is a one-line note for an agent's system prompt.

## The familiar syntax

`var x = e` infers its type, and a `let` may be assigned later. The syntax also gains:

- compound assignment;
- generator folds (`sum(e for x in xs if c)`, `count`, `any`, `all`, `max`, `min`);
- a bare last value as the result;
- `if c { a } else { b }` as a value;
- `&&`, `||` and `!`;
- `break` and `continue`.

`help card` is a compact reference. The default guide points to both.

## Structured bodies

New forms in the structured-body language, each lowered to existing statements and operations:

- `fold`, `floordiv`, `floormod`, `truthy`, `pow`, `powmod`, `gcd`;
- an untyped `var`, `declare`, `break` and `continue`.

## Compatibility

Programs written for 2.1.0 keep their meaning. Some familiar texts that 2.1.0 refused (for
example `x += 1` or `let mut`) are now accepted. Schema epoch, SSMC1, SCB1, SMP1, validation
semantics, the instruction set and the protocol method table are unchanged. No new third-party
crate is added. `ga_claimed` remains `false`.

## Candidate packaging

The archive is `sley-2.1.1-linux-x86_64.tar.gz`. It contains static musl Linux x86_64 `sley`
and `sley-agent` binaries. From a clean checkout:

```sh
cargo fetch --locked
python3 scripts/build_release_candidate.py --require-clean
sha256sum dist/sley-2.1.1-linux-x86_64.tar.gz
```

The Linux x86_64 archive was built from `af34be33eb1ae913dddd92aa7f4f275aac0e45ee`. Two clean
builds on the primary host produced identical bytes. Cross-host reproducibility is not claimed.
The archive SHA-256 is `b0898ba7404a1656db2f83f448ea96d2c1175346f35e50b135bedc96e3810bfd`
(7,413,689 bytes, 17 members). Provenance remains unsigned.
