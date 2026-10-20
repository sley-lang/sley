# Sley 2.1.2 release notes

Sley 2.1.2 is a point release. It ships the work merged since 2.1.1: a Python-dialect fix, a
storage fix, clearer reporting of what the advisory test runner applies, the native test
supervisor source with its diagnostics and limits specified, and restored maintainer gates.
What a committed program means is unchanged: the kernel, SSMC1, SCB1, SMP1 rules, the
instruction set, admission and commit semantics are untouched.

## Fixes

- **Python tuple assignment to parameters** (#39). `a, b = b, a % b` in a function whose
  parameters are `a` and `b` was refused with `AGENT_FRAME_INVALID` ("set needs a var in
  scope"), although the 2.1.1 notes list tuple assignment and assignable parameters. The
  dialect now reads every name of a tuple target list, including in loop bodies. Euclid's gcd,
  swaps, rotations and loop forms are covered by a regression test checked against CPython.
  `help card` and `help familiar` no longer call `break` and `continue` refused, and the
  float refusals now say the *text frontends* have no floating-point syntax (AF1 frames can
  use `f32`/`f64`).
- **Candidate handle exhaustion** (#28). Saving a candidate after the last `u64` handle was
  occupied panicked. It now returns `AGENT_IO_FAILED` and leaves existing candidate bytes and
  metadata unchanged. SLEY_AGENT_V1 states the recovery boundary for a failed metadata write;
  this is not atomic two-file publication or a power-loss durability guarantee.

## Test reporting

`try` (including draft follow-ups), `test` and `status` now report the advisory TestCase
execution policy (#29). Declared `fuel` is applied as VM fuel. Declared `memory_bytes`,
`output_bytes`, `effect_count`, `call_depth` and `wall_timeout_millis` are reported as not
applied, and the fixed VM caps are listed. JSON gains a top-level `test_execution_policy`
object (`null` when no TestCase ran); text prints the same policy once after the results. A
passing test still means only that the expected value or trap matched. The new
[guarantee matrix](../GUARANTEES.md) separates validity, advisory matches, submission and
commit.

## Native test supervisor (source only)

The native test supervisor and the receiver's explicit native commit authority are integrated
in source (#23). `sley serve --protocol-profile v3-capable` accepts
`--native-authority-config`; without it, the default, v1 and v2 routes are unchanged. In 2.1.2
the supervisor's 106 private diagnostic symbols, the nine `NATIVE_CLI_*` startup causes and the
runner's seven size and time limits are specified ([NATIVE_TEST_EXECUTION_V1](../spec/NATIVE_TEST_EXECUTION_V1.md)
section 11, [SLEY_CLI_V1](../spec/SLEY_CLI_V1.md) section 11), with meanings taken from the code
paths. The 28 refusal paths that no test reached now have tests, adversarial where the path
guards privileged code (#42). No supervisor defect was found; one test race was fixed. The
native-worker conformance vector is now checked independently at `codec_and_identity` depth;
it makes no semantics claim, because the vector is derived from the Rust VM.

The release archive does not contain the supervisor or its service unit, and this build has not
been qualified on an installed host with real cgroups. With no qualified supervisor, committing
a candidate that selects TestCases still refuses with `TXN_TEST_EVIDENCE_UNSUPPORTED`, as in
2.1.1. Independent review of the supervisor's security contracts is still advisable.

## Documentation and evidence

- The README leads with a first authoring example, and two Make targets check the release entry
  points against the workspace version (#33, [PUBLIC_ENTRYPOINTS](../PUBLIC_ENTRYPOINTS.md)).
- A phase-separated scaling probe with its retained samples (#36,
  [SCALING-PHASES](../bench/SCALING-PHASES.md)), an offline package that reproduces the
  arithmetic of the 2.1.1 authoring-cost figures from recorded values (#37), and an initial
  workload and adoption roadmap (#38, [ADOPTION_ROADMAP](../ADOPTION_ROADMAP.md)). The arithmetic
  package does not independently verify models, prices or the released runtime.
- `make contributor-check` is one clone-reproducible gate: formatting, strict Clippy, workspace
  tests, conformance, adversarial tests and bounded fuzz smoke (#27).
- Maintainer gates broken since 2.1.1 were restored (#41, #42), and a parallel-test temp
  directory collision was fixed (#40).

## Compatibility

Programs written for 2.1.1 keep their meaning, and some Python-dialect texts that 2.1.1 refused
are now accepted. Schema epoch, SSMC1, SCB1, SMP1, validation semantics, the instruction set and
the protocol method table are unchanged. Strict consumers of `sley-agent` JSON may need to
accept the added `test_execution_policy` field. The binaries now link four new third-party
crates through `sley-test-runner`, which the native admission path uses: `nix` and `memoffset`
(MIT), `bitflags` and `getrandom` (MIT or Apache-2.0). `cfg_aliases` (MIT) and `autocfg` (MIT or
Apache-2.0) are build-time only, and `r-efi` (MIT, Apache-2.0 or LGPL-2.1-or-later, taken under
MIT) is a UEFI-only lock entry that is not compiled for Linux. `cargo audit` reported no
advisories for either lockfile (RustSec database of 2026-10-17). `ga_claimed` remains `false`.

## Waived checks

Operator decision 2026-10-19 (Magister): for Sley 2.1.2, `scripts/check_candidate_result_contract.py`
and `scripts/check_transaction_contract.py` are not run because the Sley 2.0 master goal
(`SLEY2_MASTER_GOAL`) is unavailable off the maintainer host. This waives the two checks for this
release only and is not a pass; both passed at v2.1.1 on the maintainer host. The waiver is also
recorded in the machine summary's `release_decision`.

## Candidate packaging

The archive is `sley-2.1.2-linux-x86_64.tar.gz`. It contains static musl Linux x86_64 `sley`
and `sley-agent` binaries. From a clean checkout:

```sh
cargo fetch --locked
python3 scripts/build_release_candidate.py --require-clean
sha256sum dist/sley-2.1.2-linux-x86_64.tar.gz
```

Checksums, the exact source commit, test results and any host attestations must identify the
delivered 2.1.2 artifact. Records from earlier releases do not qualify it.
