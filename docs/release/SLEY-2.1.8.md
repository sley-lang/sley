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

## Benchmarks

These are development measurements of `sley-agent call` on the eleven small integer programs of
the 2.1.7 suite. They are not a GA claim and say nothing about typical workloads. Both Sley
columns are the static musl `sley-agent` binaries from the 2.1.7 and 2.1.8 release archives,
measured with CPython 3.13.5 in one session on one 8-vCPU cloud VM, pinned to one vCPU, in three
rounds with the command order rotated (2 warmups and at least 10 runs per command per round).
Other runtimes were not re-measured; the 2.1.7 notes have them.

**Plainly:**

- **Against CPython 3.13**, with process start included, 2.1.8 is faster on all eleven programs
  in this session (0.37–0.92x of CPython's time). In 2.1.7 it was slower on digitsum; it is now
  2.6x faster there (19.2 vs 50.1 ms).
- **Against 2.1.7**, five programs are faster because they use non-negative `//` or `%`:
  digitsum 2.83x, collatz 1.90x, primes 1.83x, isqrt 1.41x and loopsum 1.12x. Their VM
  instruction counts drop by 18–53% with the same results.
- **Unchanged:** fib, gcdsum, tak, triples and both inversion counts run the same instructions
  as 2.1.7, and their wall times are within 4% of 2.1.7 in either direction (triples 0.96x, fib
  1.02x), which this session cannot tell apart from noise.
- **Not fixed:** the `fib(30)` regression from 2.1.6 (about 4% in the 2.1.7 notes) was not
  re-measured against 2.1.6 here; fib's instructions are unchanged from 2.1.7.
- C, Rust, Go, LuaJIT and Lua 5.4 remained faster on every program in the 2.1.7 session, and
  nothing in 2.1.8 changes that.

Wall time in ms, median [min–max] over all runs of the three rounds:

| benchmark | Sley 2.1.7 | Sley 2.1.8 | 2.1.7 / 2.1.8 | CPython 3.13.5 | 2.1.8 / CPython | VM instructions 2.1.7 → 2.1.8 |
|---|---|---|---|---|---|---|
| fib 30 | 78.3 [74.6-117.5] | 77.0 [75.3-170.1] | 1.02x | 84.0 [80.8-173.2] | 0.92x | 14,808,950 → 14,808,950 |
| loopsum 1e6 | 34.9 [34.0-83.3] | 31.1 [29.6-48.0] | 1.12x | 58.8 [56.8-138.3] | 0.53x | 11,000,003 → 9,000,003 |
| collatz 5000 | 27.5 [26.8-32.9] | 14.4 [14.0-81.8] | 1.90x | 35.9 [34.9-58.6] | 0.40x | 7,019,817 → 4,171,757 |
| primes 30000 | 28.5 [27.5-93.9] | 15.5 [15.0-28.7] | 1.83x | 41.5 [40.6-47.1] | 0.37x | 7,995,140 → 3,803,716 |
| gcdsum 400 | 26.0 [25.3-41.1] | 26.1 [25.3-44.1] | 0.99x | 42.6 [41.7-93.1] | 0.61x | 4,361,274 → 4,361,274 |
| tak 24 16 8 | 70.2 [68.0-95.8] | 70.3 [68.3-111.7] | 1.00x | 80.3 [78.6-134.6] | 0.88x | 8,726,719 → 8,726,719 |
| triples 120 | 12.4 [11.8-32.3] | 12.9 [12.1-38.0] | 0.96x | 26.1 [24.8-40.2] | 0.49x | 2,283,157 → 2,283,157 |
| digitsum 150000 | 54.3 [52.9-62.9] | 19.2 [18.5-37.0] | 2.83x | 50.1 [48.0-63.3] | 0.38x | 13,372,227 → 6,272,226 |
| isqrt 50000 | 40.5 [39.7-48.5] | 28.8 [28.2-36.9] | 1.41x | 58.4 [56.8-93.1] | 0.49x | 9,771,330 → 6,918,614 |
| inversions 1000 | 20.3 [19.6-26.8] | 20.6 [19.9-32.2] | 0.99x | 30.6 [29.5-65.5] | 0.67x | 4,007,971 → 4,007,971 |
| inversions 400 | 8.9 [8.3-79.0] | 8.8 [8.3-45.6] | 1.00x | 17.3 [16.6-57.5] | 0.51x | 646,212 → 646,212 |

Results match between versions on every program, and fuel and instructions match
`call --reference`. Records and the script are in `evidence/bench/runtime/2.1.8/`.

## Soundness gate for the `//` and `%` fast path

The 2.1.8 plan asked for a differential fuzz test of the fast path before release. It was run
as a CPython differential: `evidence/bench/runtime/2.1.8/s1-fuzz/gen.py` writes random
Python-dialect programs mixing signs, `//`, `%`, range counters starting at or below zero,
values reassigned negative after a provably non-negative start, and `while x > 0` digit loops.
Seeds 1–200 gave 8,000 assertions against CPython's answers with 0 mismatches. This compares
against CPython, not against a forced slow path in the same binary. The general VM differential
fuzzer (#101) landed on `main` after the cut and is not part of this release.

## Token cost

The scripted write-test-fix token loop from 2.1.7 was not re-run, so 2.1.8 makes no token-loop
claim. Refusal text changed (#75), which changes the feedback tokens in that loop's error
scenarios, and `help python` grew from 407 to 492 o200k tokens (#82, #83).

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

## Candidate packaging

The archive is `sley-2.1.8-linux-x86_64.tar.gz`. It contains static musl Linux x86_64 `sley`
and `sley-agent` binaries. From a clean checkout of `v2.1.8`:

```sh
cargo fetch --locked
python3 scripts/build_release_candidate.py --require-clean
sha256sum dist/sley-2.1.8-linux-x86_64.tar.gz
```

The Linux x86_64 archive was built from `3c5b7119c34642abb49480a776bd527ed5f5bec0`. Two clean
builds on the primary host produced identical bytes. Cross-host reproducibility is not claimed.
The archive SHA-256 is `399e2220b2da0c1561bd5b6e210eea6d06a2062fc3524322dc1a2b9f6ef5cf47`
(7,699,874 bytes, 17 members). Provenance remains unsigned. `make release-candidate-verify`
passes on the recorded candidate.

**Not refreshed:** the persistent-fuzz proofs were not re-run for 2.1.8, because the pinned
fuzzing toolchain (Debian clang 18.1.8) was not available on the build hosts. The tracked proofs
are the 2.1.7 ones, run at `f5ef7125`. The code those lanes cover (kernel codecs, schema, pack,
query, VM and protocol parsers) is unchanged since 2.1.7; 2.1.8 changes only the Python-dialect
lowering and refusal text in `sley-agent`, the help cards and the benchmark harness. The GA
acceptance report is unchanged from 2.1.7: of 52 criteria, 31 are evidenced, 17 await review and
4 are gated.
