# Sley 2.1.4 release notes

Sley 2.1.4 is a point release with two parts. The VM's compact scalar plan runs loops faster,
with fuel, instruction, value-unit and output-unit accounting unchanged. The Python dialect and
`sley-agent try` cut the tokens an agent spends in a write-test-fix loop. The kernel, SSMC1, SCB1,
SMP1 rules, the instruction set, admission and commit semantics are untouched.

## VM performance (#49)

- **Slot coalescing.** A liveness pass gives each register a slot. Registers joined by a block
  edge share a slot when the blocks they are live in are disjoint, so most edge moves become
  no-ops. This is the main gain for Python-dialect loops, which thread every live value through
  each checked operation's continuation.
- **Prepaid fuel batches.** Each block is cut into runs of steps between calls. When a run's
  worst-case fuel, instruction count and value units fit inside every limit, the run skips its
  per-step checks and adds the exact totals once. A fault partway through a run fixes the
  counts up to what the per-step path would have charged.
- **Word results built whole.** The 64-bit fast path builds each value in one store, which
  removes two store-forwarding stalls.

Fuel, instruction counts and value units are identical to `call --reference`. A new agent-level
test checks this on the five benchmark programs at several sizes, and a differential test covers
the word fast path on edge values.

## Python dialect and `try` output (#50, #51)

- **Inline pytest-style tests.** Python-dialect files may contain `def test_*():` functions whose
  lines are `assert f(...) == value`, `assert f(...)`, `assert not f(...)` or
  `with pytest.raises(E): f(...)`. They become TestCases in the same frame, so one
  `try --familiar` compiles and tests the file. `pytest.raises(E)` is checked statically: the
  called function, or a function it calls in the file, must be able to raise `E`. At run time
  any trap still satisfies it, because the dialect lowers every exception to one trap.
- **Record classes.** A class of annotated fields (`@dataclass class Point: x: int`) declares a
  record type. Before, the class was skipped and the type needed a separate AF1 `types` frame.
  Other classes are still skipped.
- **Shorter `try` output.** A Valid candidate whose tests all pass prints one line and `next:`.
  With tests, the advisory test policy fits on one line: still advisory, not admission evidence,
  and only fuel and the fixed VM limits apply. `changed:` collapses constant and numbered-test
  runs. A failing test from familiar or dialect text names its source line (`(line N)`), and
  `next:` says to fix the text and re-run `try --familiar`. `--verbose`, `--json`, `test` and
  `status` still print the full report and policy.
- **Token harness.** [`bench/token-cost`](../../bench/token-cost) is the scripted harness
  behind the token table below.

## Compatibility

Programs written for 2.1.3 keep their meaning and their answers, and their fuel and instruction
counts are unchanged. Two Python-dialect changes can alter how existing text compiles:

- a `def test_*():` function made of the assert forms above now becomes TestCases instead of a
  function;
- a class of annotated fields now declares a record type instead of being skipped.

The human-readable `try` output is shorter. Scripts should read `--json`, which is unchanged. Schema epoch, SSMC1, SCB1, SMP1, validation semantics,
the instruction set and the protocol method table are unchanged. No third-party dependency was
added to the binaries. `ga_claimed` remains `false`.

The native test supervisor is still source only, as in 2.1.2. It is not in the release archive and
not qualified on an installed host, so committing a candidate that selects TestCases still
refuses with `TXN_TEST_EVIDENCE_UNSUPPORTED`. The inline tests are advisory matches, like every
`try` test.

## Waived checks

Carried forward from the operator decision of 2026-10-19 (Magister) for Sley 2.1.2. The operator
has not re-confirmed it for 2.1.3 or 2.1.4. For Sley 2.1.4,
`scripts/check_candidate_result_contract.py` and `scripts/check_transaction_contract.py` are not
run because the Sley 2.0 master goal (`SLEY2_MASTER_GOAL`) is unavailable off the maintainer
host. This waives the two checks for this release only and is not a pass; both passed at v2.1.1
on the maintainer host. The waiver is also recorded in the machine summary's `release_decision`.

## Candidate packaging

The archive is `sley-2.1.4-linux-x86_64.tar.gz`. It contains static musl Linux x86_64 `sley`
and `sley-agent` binaries. From a clean checkout:

```sh
cargo fetch --locked
python3 scripts/build_release_candidate.py --require-clean
sha256sum dist/sley-2.1.4-linux-x86_64.tar.gz
```
