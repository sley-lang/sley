# Sley 2.1.3 release notes

Sley 2.1.3 is a performance point release. The VM runs call-heavy and loop-heavy programs
several times faster than 2.1.2. Fuel, instruction counts, value-unit and output-unit accounting
are unchanged, and so is what a committed program means: the kernel, SSMC1, SCB1, SMP1 rules,
the instruction set, admission and commit semantics are untouched.

## Performance

- **Direct calls in the compact scalar plan** (#46). The VM's compact scalar plan used to refuse
  any function containing `call_direct`, so every recursive or call-heavy program ran on the
  general reference interpreter. The plan now compiles the entry function and every function it
  reaches. If any of them is ineligible, the whole execution keeps the reference path, as before.
  Calls follow contract E6 exactly as the reference interpreter runs them: the depth ceiling is
  checked before the call action, each argument's value units are charged, the callee's return
  is measured against the output limit, and the returned value's units and the call instruction
  are charged on return. Frames live on an explicit stack, not on host recursion, and returned
  register files are reused. A new test sweeps every fuel, instruction, cancellation,
  value-unit and output cut for a direct call, a nested call and unbounded self-recursion
  (`CallDepth`), and compares the plan's observations with the reference interpreter's.
- **Faster scalar loops** (#45, #46). The plan has a native fast path for 64-bit checked
  arithmetic, checked against the general 128-bit path in a differential test. It moves
  block-edge arguments directly when they cannot overlap, and its registers are 32 bytes
  instead of 48, with no padding. A register's one-byte shape used to leave 7 padding bytes,
  which the compiler copied as overlapping moves that stalled on store forwarding.
- **Python `%` and `//`** (#44). The dialect computed the remainder up to three times per
  operation and tested a literal divisor's sign at run time. It now holds one remainder and
  folds a literal divisor's sign. Results, traps (`/ 0`, `MIN // -1`) and CPython floor
  semantics are unchanged; fewer instructions and less fuel are spent.

## Compatibility

Programs written for 2.1.2 keep their meaning and their answers. A program's fuel and
instruction counts are unchanged, except that Python-dialect programs using `%` or `//`
compile to fewer instructions and spend less fuel. Schema epoch, SSMC1, SCB1, SMP1, validation
semantics, the instruction set, the protocol method table and the `sley-agent` JSON shape are
unchanged. No third-party dependency was added. `ga_claimed` remains `false`.

The native test supervisor is still source only, as in 2.1.2. It is not in the release archive and
not qualified on an installed host, so committing a candidate that selects TestCases still
refuses with `TXN_TEST_EVIDENCE_UNSUPPORTED`.

## Waived checks

Operator decision 2026-10-20 (Magister), carried forward from the 2.1.2 waiver of 2026-10-19: for
Sley 2.1.3, `scripts/check_candidate_result_contract.py` and `scripts/check_transaction_contract.py`
are not run because the Sley 2.0 master goal (`SLEY2_MASTER_GOAL`) is unavailable off the
maintainer host. This waives the two checks for this release only and is not a pass; both passed
at v2.1.1 on the maintainer host. The waiver is also recorded in the machine summary's
`release_decision`.

## Candidate packaging

The archive is `sley-2.1.3-linux-x86_64.tar.gz`. It contains static musl Linux x86_64 `sley`
and `sley-agent` binaries. From a clean checkout:

```sh
cargo fetch --locked
python3 scripts/build_release_candidate.py --require-clean
sha256sum dist/sley-2.1.3-linux-x86_64.tar.gz
```
