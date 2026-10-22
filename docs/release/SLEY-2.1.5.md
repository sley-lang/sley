# Sley 2.1.5 release notes

Sley 2.1.5 is a point release with two parts. The VM gains a **word tier** for programs whose
values all fit a machine word, and the release binary is built with fat LTO. Fuel,
instruction, value-unit and output-unit accounting is unchanged. `sley-agent try` prints less,
familiar text is refused with compiler-style diagnostics, and familiar syntax declares `struct`
and `enum` types. The kernel, SSMC1, SCB1, SMP1 rules, the instruction set, admission and
commit semantics are untouched.

## VM performance (#54, #56, #57)

- **Word tier (#54).** When every register of every function is an `i64`/`u64`, a `bool`, a
  checked result read only by its block's switch, or that result's failure code, the scalar
  plan runs on plain `u64` registers in one contiguous register file. Signedness is resolved at
  compile time, and checked arithmetic branches straight to its Err edge. Up to 64 whole blocks
  run back to back against one prepaid worst case. Any other program keeps the 2.1.4 scalar loop.
- **Fused fast blocks (#56).** Constants are written once per frame. A fast block continues
  through unconditional branches, a checked switch's Ok edge and loop exits, with a guard on
  each; a failed guard stops with exactly the charges of the blocks already run.
- **Calls and returns inside bursts (#57).** A direct call, the callee's frame and the return
  run inside the burst, with the same charges in the same order as the general loop. So
  `down(255)` runs and `down(256)` stops at `CallDepth`, as in 2.1.4.
- **Release profile (#57).** `[profile.release]` now sets `lto = "fat"` and
  `codegen-units = 1`. The two clean release builds still produce identical archives (see
  [Candidate packaging](#candidate-packaging)).

Fuel and instruction counts are identical to 2.1.4 and to `call --reference` on all five
benchmark programs. New differential tests compare the word tier with the reference runner and
the scalar loop at every fuel, cancel, instruction, value-unit and output cut, including
two-argument call chains to depth 400. Mutation testing on the three PRs killed every mutant
tried (6, 6 and 22).

## Agent output and familiar syntax (#55)

- **Quiet `try`.** A valid candidate whose tests ran prints its failures (familiar text only,
  each with its source line) and one count line naming the candidate and draft, for example
  `c2: 2 passed (advisory) d2@r1`. Test provenance is shown only when tests other than the
  authored ones ran. Replaced or deleted entities are still listed.
- **Compiler-style refusals.** Familiar text refused at a source position prints
  `file:line:col: message`, the source line and a caret, then the draft state. Further
  obligations are counted as `(+N more; --verbose lists them)`.
- **`struct` and `enum` in familiar syntax.** `struct Point { x: i64, y: i64 }` and
  `enum Error { Empty, TooBig }` declare the AF1 `types` entries an AF1 frame would, before or
  between functions. Payload variants, repeated members and empty declarations are refused at
  their position.
- **Token harness.** [`bench/token-cost`](../../bench/token-cost) gains C, Rust, Go and
  TypeScript versions of every task, run with their usual test runners.

## Compatibility

Programs written for 2.1.4 keep their meaning and their answers, and their fuel and instruction
counts are unchanged. The human-readable `try` output and the default text of a familiar
refusal are shorter. `--verbose` and `--json` are unchanged and still give the full report,
the advisory policy, `next:` and every pointer; scripts should read `--json`. A familiar
declaration starting with `struct` or `enum` now declares a type. Schema epoch, SSMC1, SCB1,
SMP1, validation semantics, the instruction set and the protocol method table are unchanged. No
third-party dependency was added to the binaries. `ga_claimed` remains `false`.

The native test supervisor is still source only, as in 2.1.2. It is not in the release archive and
not qualified on an installed host, so committing a candidate that selects TestCases still
refuses with `TXN_TEST_EVIDENCE_UNSUPPORTED`.

## Waived checks

Carried forward from the operator decision of 2026-10-19 (Magister) for Sley 2.1.2. The operator
has not re-confirmed it for 2.1.3, 2.1.4 or 2.1.5. For Sley 2.1.5,
`scripts/check_candidate_result_contract.py` and `scripts/check_transaction_contract.py` are not
run because the Sley 2.0 master goal (`SLEY2_MASTER_GOAL`) is unavailable off the maintainer
host. This waives the two checks for this release only and is not a pass; both passed at v2.1.1
on the maintainer host. The waiver is also recorded in the machine summary's `release_decision`.

## Candidate packaging

The archive is `sley-2.1.5-linux-x86_64.tar.gz`. It contains static musl Linux x86_64 `sley`
and `sley-agent` binaries. From a clean checkout:

```sh
cargo fetch --locked
python3 scripts/build_release_candidate.py --require-clean
sha256sum dist/sley-2.1.5-linux-x86_64.tar.gz
```
