# Sley 2.1.7 release notes

Sley 2.1.7 is a performance point release. Python-dialect list indexing by a loop counter no
longer converts the index on every access, and the VM's word tier now runs vector length and
element reads and fuses more of each burst. Program meaning is unchanged, and fuel, instruction,
value-unit and output-unit accounting match `call --reference`. The kernel, SSMC1, SCB1, SMP1
rules, the instruction set, admission and commit semantics are untouched.

## Python dialect: loop-counter index shadow (#69)

`xs[i]` with an `int` (`i64`) loop counter used to lower through a `to(u64, i)` conversion on
every access. A `range` loop whose body converts its counter now carries a `u64` copy that steps
with the counter, and offsets such as `xs[i + 1]` or `xs[i - 1]` read from it directly. An
`enumerate` loop gets an `i64` copy of its index. Out-of-range and negative indexes still fail as
before. A new test compares each fast program with a twin that defeats the shadow, and checks
that results match (failing cases included), that fuel and instructions match
`call --reference`, and that the fast twin runs fewer instructions.

## VM: vector reads on the word tier and more fusion per burst (#70)

- **Vector reads.** `Vec<word>` arguments and `Option<word>` values are word-tier types. The new
  word ops `VecLen` and `VecGet` read a vector in place, and `Some`/`None` switches map onto the
  Ok and Err edges. A function that uses a vector in any other way keeps the 2.1.6 path.
- **More fusion.** A conditional now fuses one side whenever either side can fuse. Guards merge
  into the step that produced the guarded slot, and edge moves merge into their producer step.
  Moves into slots that are never read are dropped.

A new differential test runs vector reads at budgets of 1, 2, 3 and unlimited over empty,
single-element, mixed-sign and min/max vectors, against the reference interpreter.

BENCHMARKS_PLACEHOLDER

## Compatibility

Programs written for 2.1.6 keep their meaning and their answers. Fuel and instruction counts are
unchanged for every program in the speed suite except those that index lists by a loop counter,
which now spend less: the inversion count over 400 elements finishes in 8,203,516 fuel and 646,212
instructions, where 2.1.6 stops at the 100,000,000 call fuel limit. Both match
`call --reference`. Schema epoch, SSMC1, SCB1, SMP1,
validation semantics, the instruction set and the protocol method table are unchanged. No
third-party dependency was added to the binaries. `ga_claimed` remains `false`.

The native test supervisor is still source only, as in 2.1.2. It is not in the release archive
and not qualified on an installed host, so committing a candidate that selects TestCases still
refuses with `TXN_TEST_EVIDENCE_UNSUPPORTED`.

## Design-specification checks

For Sley 2.1.7, `scripts/check_candidate_result_contract.py` and
`scripts/check_transaction_contract.py` were not run, because the Sley 2.0 design specification
(`SLEY2_DESIGN_SPEC`) is unavailable off the maintainer host. This uses the waiver carried forward
from the operator decision of 2026-10-19 (Magister) for 2.1.2; the operator has not re-confirmed
it for 2.1.7. It waives the two checks for this release only and is not a pass. Both passed at
v2.1.6 on the maintainer host. The machine summary's `release_decision` records this.

PACKAGING_PLACEHOLDER
