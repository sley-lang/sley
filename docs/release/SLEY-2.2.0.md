# Sley 2.2.0 release notes

Sley 2.2.0 adds native Text operations, Python-dialect strings, bisect, integer dicts and sets, guard-derived non-negative division, automatic Python input detection and explicit with-card token accounting. `call --fuel` can lower, never raise, the existing ceiling. There is no new measured runtime or token-cost headline.

The executable cut is `1d976f895759e1c1aa072d0cc0e15bb6c52d4559` (#114). The release tag also contains these finalized notes and candidate evidence; those records do not change the executable sources.

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

## CLI input detection and token accounting

T1 (#112) automatically detects Python-shaped try input, preserving JSON and explicit familiar handling. T6 (#113) adds explicit with-card totals, once per task session, preserving historical no-card data and unavailable legacy card totals.

## Deferred work

- **Independent E9 review is deferred/owed for this 2.2.0 cut.** Extended profile revision 18 records: "E9 has had self-review only; an independent review is owed." No independent-review PASS or transcript is claimed.
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

## Verification and operator acceptance

On 2026-11-05 the operator accepted the documented remaining verification items and deferrals for shipping 2.2.0:

- The design-specification checks (`check_candidate_result_contract.py` and `check_transaction_contract.py`) use the carried-forward 2.1.2 waiver, as in 2.1.8. They are not passes. The original waiver had not been re-confirmed during draft preparation; carrying it forward for this cut was explicitly accepted on 2026-11-05. The candidate-result attempt reported `master:unavailable:.:set-SLEY2_DESIGN_SPEC`.
- Persistent-fuzz proof records remain the 2.1.7-era records where the covered code is unchanged. No fresh persistent-fuzz proof is claimed for changed code, including the Text VM work.
- No fresh runtime benchmark session is required for this cut. The fib disclosure above is historical, not a new 2.2 measurement.
- Measurement-v2/four-arm API runs remain blocked and were not run. Held-out tasks were not sealed and no preregistration hash was computed. These are not release-completion claims.

Release-preparation verification recorded 1,099 passing sley-agent release tests, including Python 32/32, clean all-target clippy with warnings denied, and 9/9 offline token-card tests. Four local oracle regressions and 2,343 exhaustive small-domain cases passed; this is not a full oracle sweep or a claim that every workspace/conformance suite was freshly run against the final tag. `cargo fmt --all --check` passed on the executable cut.

## Candidate packaging

The archive is `sley-2.2.0-linux-x86_64.tar.gz`. It contains static musl Linux x86_64 `sley`
and `sley-agent` binaries, the offline conformance subset and the source-independent demo.
From a clean checkout of the release commit:

```sh
cargo fetch --locked
make release-candidate-build
sha256sum dist/sley-2.2.0-linux-x86_64.tar.gz
```

The Linux x86_64 archive was built from
`bc9da8c76b298ff4b399b9c12d862e8ee89b17d5`. Two clean builds on the primary host
produced identical bytes. Cross-host reproducibility is not claimed. The archive
SHA-256 is `7a601f10ad947716f47d8ad16c7bfc95d2109ba56674b0f5a9215e9b0fb947b1`
(7,887,554 bytes, 17 members). `make release-candidate-verify` passes on the
recorded candidate.

Provenance remains unsigned; cross-host reproducibility is not claimed. This is not a GA claim: `ga_claimed` remains `false`. The operator authorized tagging and publishing on 2026-11-05.
