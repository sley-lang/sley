# Sley 2.1.8 release notes

Sley 2.1.8 is a point release of work that is already built and measured. Python `//` and `%`
on values the compiler can prove non-negative lower to plain divide and remainder (#73), which
makes digitsum about 2.9x faster. Python-dialect refusals now name the fix (#75). A shorter
`help python-task` card (#82, #83) is written for tasks whose tests come from elsewhere. And the
repository now carries a genuine model-cost benchmark (#74, #76 to #84) with its first results,
published as measured: **on Claude Haiku 5.5, Sley costs about 1.25x Python per trial**, with
every language near 100% correct. Program meaning is unchanged, and fuel, instruction,
value-unit and output-unit accounting match `call --reference`. The kernel, SSMC1, SCB1, SMP1
rules, the instruction set, admission and commit semantics are untouched.

2.1.8 is cut from `4da66c0` (#84 merged) on the `release/2.1.8` branch. Work merged to `main`
after that point (growable lists #88/#89, list results #95, native Text operations #102, the
Python `str` operations #103 and task set v2 #91 to #104) belongs to Sley 2.2 and is not in
this release.

## Python dialect: non-negative `//` and `%` (#73)

`//` and `%` used to lower to `floordiv`/`floormod`, which carry a sign fixup so negative
operands floor the way Python does. When both operands are provably non-negative, the lowering
now emits plain `div`/`rem`. The proof is a per-function analysis over binding sites (`let`,
`set` and `var` values, range counters bounded below by their start, `for`-indexed indexes and
fold variables). Parameters are never assumed non-negative. A pair such as `q = x // 10;
r = x % 10` drops from three divisions plus fixups to two plain ones.

A twin test checks each fast program against a slow twin that defeats the proof: results match
each other and `call --reference`, the fast twins run fewer instructions, and programs that go
negative keep Python's floor semantics. A random dialect fuzzer compared against CPython over 37
seeds and 1,776 cases with no mismatch. On digitsum 150000, VM instructions fall from 13,372,227
to about 6.27 million for the same result.

## Python dialect: refusals that name the fix (#75)

The gaps models hit most often now say what to write instead: list methods such as
`.append()`, dicts and sets, `.count()`/`.index()`, `return None`, `print`, `[]` and slices as
values, and built-ins such as `sorted`, `str`, `ord` and `map`. What the dialect accepts is
unchanged; only refusal text changed. This change has no before-and-after measurement: the 2.1.7
baseline was never taken.

## `help python-task` and "instead of" lines (#82, #83)

`sley-agent help python-task` is `help python` without the lines on writing inline tests, for
tasks whose tests come from elsewhere. Both cards say that only parameters and returns need
types, and both carry two "instead of" lines (counting instead of `Counter`, a manual bisect,
a trial-division prime test instead of a `[v] * n` sieve, and clamping with `min`/`max`).
Sizes in o200k tokens: `help python-task` 349, `help python` 492 (the 2.1.7 `help python` was
407).

## Model-cost benchmark (#74, #76 to #84)

`bench/model-cost/` measures dollars per correct program when a model writes each task in
Python, Sley (Python dialect), C, Rust, Go or TypeScript. An independent oracle checks every
answer on 128 seeded hidden cases. The runs below used Claude Haiku 5.5 through the Claude Code
CLI at low effort. Tokens are as reported by Claude Code; dollars are at API list price, not a
bill.

| run | trials | correct | est. total | Sley / Python per trial (95% CI) | Sley − Python per trial |
|---|---|---|---|---|---|
| full tier, 2.1.7 card (#81) | 324 | 305 as scored, 323 with the fixed collatz oracle | $0.119 | 1.20x (1.04–1.41) | +59.7 µ$ |
| full tier, 2.1.8 card (#84) | 324 | 324 | $0.132 | 1.25x (1.10–1.45) | +77.9 µ$ |

Per-trial ratios against Python in #84: TypeScript 1.15x, Rust 1.20x, Sley 1.25x, Go 1.41x,
C 1.69x. The confidence intervals come from a paired bootstrap over tasks added after the cut
(#85, on `main`). A raw-API run with the same card (#90, on `main`, $0.05169 billed) measured
Sley at 1.27x Python (95% CI 0.91–1.78), TypeScript 1.47x, Rust 1.72x, Go 1.96x and C 2.74x.

What this does and does not show:

- **Sley is not cheaper than Python on this set.** It costs about 20–25% more per trial. The
  gap is hidden thinking (91% of Sley calls think in #84, against 37% for Python), not retries
  (2 of 54 Sley trials retried in #81), and Sley's visible output costs less than Python's.
- **The new card did not close the gap.** #84 against #81 is a change smaller than one run pair
  can detect. The planned A/B of #82 alone against #82 + #83 in one run was not done, so the
  "instead of" lines ship without evidence that they help; they coincided with better
  count_leq and parse_int and worse mode_value, second_largest and primes.
- **The collatz oracle was wrong for `limit = 2`** in #81 (#80). Both scores are published.
- One model, one provider, 18 tasks, three repeats. This is not a general cost claim.

Raw records, manifests and summaries are under `evidence/bench/model-cost/2.1.8/`.

BENCHMARKS_PLACEHOLDER

## Compatibility

Programs written for 2.1.7 keep their meaning and their answers. Fuel and instruction counts
are unchanged except for programs that apply `//` or `%` to provably non-negative values, which
now spend less. Both paths match `call --reference`. Schema epoch, SSMC1, SCB1, SMP1, validation
semantics, the instruction set and the protocol method table are unchanged. No third-party
dependency was added to the binaries. `ga_claimed` remains `false`.

The native test supervisor is still source only, as in 2.1.2. It is not in the release archive
and not qualified on an installed host, so committing a candidate that selects TestCases still
refuses with `TXN_TEST_EVIDENCE_UNSUPPORTED`.

## Design-specification checks

For Sley 2.1.8, `scripts/check_candidate_result_contract.py` and
`scripts/check_transaction_contract.py` were not run, because the Sley 2.0 design specification
(`SLEY2_DESIGN_SPEC`) is unavailable off the maintainer host. This uses the waiver carried forward
from the operator decision of 2026-10-19 (Magister) for 2.1.2; the operator has not re-confirmed
it for 2.1.8. It waives the two checks for this release only and is not a pass. Both passed at
v2.1.6 on the maintainer host. The machine summary's `release_decision` records this.

PACKAGING_PLACEHOLDER
