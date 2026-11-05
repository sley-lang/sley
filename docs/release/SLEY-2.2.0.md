# Sley 2.2.0 release notes (DRAFT)

**Status of this document: draft.** Prepared for release readiness only. No
tag, no release build, no release archive, no artifact hash, no seal and no
publishing step has been taken for 2.2.0. Numbers below that come from earlier
releases are historical evidence carried forward, clearly marked; this draft
contains no fresh 2.2 measurement claims.

Sley 2.2.0 extends what the Python dialect can say and what the VM runs
natively. The release adds admitted native Text operations; review the linked changes for profile and validation compatibility. This draft supersedes nothing and is subject to the
remaining release verification listed at the end.

## Native Text operations in the VM (#102)

`slice` over `Text` now lowers to native `E9` Text entries executed over
`adapter_invoke`, in extended profile revision 18 (ADR-0039 revised). The
kernel effects check gains Text rows, and candidate validation accepts the new
entries. Program meaning is unchanged.

## Python-dialect string operations over native Text (#103)

The Python dialect now lowers string operations to the native Text entries:
`len`, indexing, iteration, slices, `+` concatenation, `ord`, `chr`, `str(n)`
and `join`. Together with #102 this is what lets Sley express the 13 string
tasks of task set v2 (#104).

## Task-set v2 references for the 13 string tasks (#104)

Reference programs for the 13 string tasks exist under
`bench/model-cost/refs/`, replacing the three tasks previously marked
`unsupported`. Sley's 2.2 dialect can now express all 24 v2 tasks; every
reference agrees with the oracle on all visible tests and all 128 seeded
hidden cases. This is reference correctness only: **no v2 model run has been
made and no v2 cost, token or performance claim is made here** (see the
measurement section below).

## Python dialect: bisect and richer collections

- `bisect_left` and `bisect_right` lower in the Python dialect (#105).
- Structured-dialect integer dict locals: `dict`, `dput`, `dget`, `dhas` and
  `len` (#108); the Python dialect gains its integer dict syntax and carries
  `dput` keys through value branches (#109).
- Local integer sets with add, membership and length in the Python dialect
  (#111). #109 and #111 both update the `help python` card text.

No before-and-after token measurement accompanies these help-card changes;
see "Not claimed in 2.2".

## Python dialect: non-negative `//` and `%` from guards (#107)

The 2.1.8 non-negative fast path now also proves names non-negative from
`loop` and `if` guard conditions, so guarded digit loops and similar programs
qualify for plain `div`/`rem` lowering. The existing twin tests cover the new
proofs: fast programs match their slow twins and `call --reference`, and
programs that go negative keep Python's floor semantics.

## CLI: `call --fuel` lowers the fuel ceiling (#110)

`sley-agent call --fuel <n>` lowers the call's fuel ceiling to `n`, for one
call or a batch. `n` is accepted only up to the default ceiling, so the flag
can never raise it, raise a capability grant or weaken kernel validation;
execution that outlives the lowered ceiling still terminates in a `Fuel`
resource limit rather than a refusal.

**Still deferred:** `--fuel` only lowers the (100M) ceiling. Accepting values
above the default ceiling is deferred work, not part of 2.2.

## Known regression, disclosed (historical evidence)

`fib(30)` became slower between 2.1.6 and 2.1.8: about 73.0 ms against about
76.5 ms, an approximately 4.3–4.8% regression, with an identical VM
instruction count (14,808,950 unchanged across 2.1.6, 2.1.7 and 2.1.8
published sessions). Because the instruction count is identical, the
difference is not extra VM work. **This is evidence from the 2.1.6 to 2.1.8
measurement sessions carried forward, not a fresh 2.2 timing**; no 2.2 runtime
session has been run, and no 2.2 wall-time figure appears in these notes.

## Measurement v2 and model cost: not claimed

- No 2.2 token-cost, runtime or model-cost headline appears in these notes,
  and no fresh paid API run was made.
- Measurement v2 (the four-arm interleaved run) is blocked and was not run.
- The held-out tasks are **not sealed**; no sealing step was taken in this work.
- **No pre-registration hash was computed** for v2; `prereg.json` was not
  sealed. The `bench/model-cost/taskset-v2/` references (#104) are a
  correctness result only.
- T2 (further pass-output compaction) and T5 (further trim and the card A/B)
  are deferred; the A/B remains undone.

## Pending and deferred work

- **T1 (#112) and T6 (#113) are merged.**
  T1 automatically detects Python-shaped try input, preserving JSON and explicit familiar handling. T6 adds explicit with-card totals, once per task session, preserving historical no-card data and unavailable legacy card totals.
- **S2 and S4:** final bounded attempts hit their time limits without a verified
  result. They are deferred/unverified — no completion claim is
  made.
- **Long-list accounting is deferred** (cumulative full-result semantic
  charging and ObservationId binding).
- **Four local oracle regression tests and 2,343 exhaustive small-domain count_by_remainder cases passed; a full task-set exhaustive sweep is deferred.**

## Compatibility

Programs written for 2.1.8 keep their meaning and their answers. Fuel and
instruction counting change only where #107's guard proofs apply (programs
then spend less) and where `call --fuel` is used (only lowering permitted).
The native test supervisor remains source only and unqualified on an
installed host, as in prior releases. `ga_claimed` remains `false`.

## Remaining release verification (draft status, explicit)

None of the following has been done for 2.2.0:

1. Cut point: pick the cut commit after the remaining pending PRs land or do
   not (T1 #112 and T6 #113 merged).
2. Design-specification checks: `check_candidate_result_contract.py` and
   `check_transaction_contract.py` were not run for 2.2 (unavailable design
   spec off the maintainer host; the carried-forward waiver has not been
   re-confirmed for 2.2). Not a pass.
3. Full workspace/conformance suites against the final cut. The release-prep sley-agent suite, clippy and 9 offline token-card tests passed before final T1 integration; integrated verification is recorded in the PR/status log.
4. Persistent-fuzz proofs re-run or explicitly carried forward as 2.1.7-era
   records.
5. Runtime benchmark session for the release candidate, if the operator wants
   one (none exists yet; the fib disclosure above is the historical record).
6. Release build: `cargo fetch --locked`,
   `scripts/build_release_candidate.py --require-clean`, two-clean-build
   reproducibility check.
7. Archive SHA-256, provenance and release seal — not computed.
8. Measurement v2 audit steps (held-out sealing, preregistration hash,
   four-arm run) — blocked, not done.
9. Operator review and approval of this draft before any tag or publish.

## Packaging

Nothing to package yet: no candidate build was made, so no archive exists and
no hash is recorded. The packaging procedure will follow the 2.1.8 notes once
the release candidate is actually built.
