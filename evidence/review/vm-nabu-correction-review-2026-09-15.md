# VM revision-15 Nabu residual correction review

Date: 2026-09-15. Repository: `/home/dev/Work/workspaces/sley2`.
Reviewed baseline HEAD: `1fc327bb9a8c3ffb0d6934f9a97b126f30334c45` plus the revision-15 worktree changes and new `bench/review/tests/test_vm_extended_evidence.py`.

## Verdict

**The twelve open/partial items in `vm-nabu-residuals.md` are now substantively corrected. No new blocking issue was found in this correction scope.** The six previously fixed items retain their corrections, yielding closure evidence for all eighteen historical findings.

This is an independent item-by-item correction review, not a replacement transcript from the historical Nabu harness, a three-lane Council verdict, or a global VM/GA/release approval. The machine summary appropriately keeps revision-15 review pending and preserves the original Nabu token. Any subsequent status/disposition change must cite this review with that limited meaning.

## Rechecked closure matrix

Paths below are repository-relative. `spec` means `docs/spec/VM_EXTENDED_OPCODE_PROFILE_V1.md`; test source means `crates/sley-vm/src/extended_tests.rs`.

| Historical ID | Result | Verified correction |
|---|---|---|
| P1-2 | CLOSED | `spec:205` states equal canonical non-float values imply equal hashes, explicitly avoids claiming the converse is collision-free, and names canonical NaN's IEEE inequality exception. The new native test at `extended_tests.rs:2913` exercises a non-float equality/hash case and equal-hash NaNs whose `equal` result is false. It passed. |
| P1-3 | CLOSED | `spec:144` explicitly requires correctly rounded declared-format evaluation and forbids excess precision, implicit FMA contraction, FTZ and DAZ. A violating environment is nonconforming. Section 7 now defers to that normative environment instead of defining semantics by the host's behavior. This closes the specification gap; it does not claim that this review qualified every host floating-point environment. |
| P1-5 | CLOSED | `spec:234` binds fixed depth 256 to the frozen VM/profile identity and requires future request-supplied depth to enter observation-bound request limits. The existing protocol/new-lowering-profile requirement remains, with the missing explanation that a profile tag alone cannot distinguish different requested depths now explicit. Existing execution semantics and preimages are unchanged. |
| P1-6 | CLOSED | `spec:429` requires owner capability prefiltering and explains S20-360's phase-7 skip, E7a/E8 owner boundaries, and why OPCODE_UNSUPPORTED is not a general program-invalidity verdict. The current consumer actually computes `operation_analysis_supported`, skips when false, then calls the judgment entry. `scripts/check_vm_extended_opcode_profile.py:288` pins those statements and their order. This checker is a source-structure guard, not a general Rust control-flow proof. |
| P1-7 | CLOSED | The old successful 256-frame vector remains intact. New `call-direct-depth-exceeded` uses 256 call links, asserts actual native `ResourceLimit(CallDepth)` at `extended_tests.rs:3622`, emits observation/accounting, and has no success-value hash. `scripts/generate_vm_extended_fixtures.py:43` records the distinct resource outcome. The stage checker checks the explicit termination shape (`:277`), and fixture regeneration reports no drift. |
| P2-1 | CLOSED | New `map-new-duplicate-key` uses repeated key 7. The native emitter asserts actual `Result::Err` containing `BuiltinFailureKind::DuplicateKey`, code 1 (`extended_tests.rs:3640`), before freezing the result hash and observation identity. Generator metadata and stage checks preserve the distinction between a failure value and a VM resource termination. |
| P2-3 | CLOSED | `spec:237` states prospective-frame depth precedes call-entry fuel/cancellation charging and preserves previous charges. The new simultaneous-boundary test (`extended_tests.rs:2877`) sets fuel and cancellation boundary 511, verifies CallDepth wins, fuel remains 511, and completed call-instruction count is zero. It passed. The count is consistent with 255 admitted calls charging dispatch plus entry, followed by the refused call's dispatch: `255*2+1=511`. This does not claim that the refused call's earlier dispatch is free. |
| P2-5 | CLOSED | `spec:470` freezes the common `[1,0,0]` observation VM version for both admitted epoch-1 profiles. Native test `epoch_one_profiles_share_the_frozen_observation_vm_version` passed, and stage checker lines 284–287 independently pin both constants. The encoder's use of the restricted constant is therefore now an explicit common-version contract, rather than undocumented coincidence. |
| P2-6 | CLOSED | `spec:475` distinguishes shared ResourceKind tags 1–5 from restricted execution's inability to produce tag 5. The misleading “closed set unchanged” sentence is replaced with reachability language. The new native resource vector pins tag-5 observation identity; the independent bytecode oracle accepts its record shape without claiming to execute its semantics. |
| P3-1 | CLOSED | ADR-0039's current status now names revision 15, implemented E1–E6/E7a/E8 and pending correction review. Prior revision-12 acceptance is clearly historical. The stage checker cross-checks this current revision. |
| P3-2 | CLOSED | Campaign current status names revision 15 and 31 vectors, records the implemented SMP1 selector and its default, and labels the former restricted-only endpoint paragraph as superseded history. Actual vector count is 31. Earlier dated validation counts are preserved as history. |
| P3-5 | CLOSED | `current_record_problems` compares current spec/ADR/campaign revisions against the summary revision and compares the campaign's current vector count against the actual corpus. Four focused Python tests pass, including independent revision/count mismatches and an end-to-end ADR mismatch causing stage-check failure. Historical metadata remains excluded from current-field checks. |

The previously fixed IDs **P1-1, P1-4, P2-2, P2-4, P3-3, P3-4** remain closed: their equality admissibility, negative-zero rule, cell restrictions, map codec authority, operand-count wording, and ordering-type list are retained. The correction diff does not alter the VM runtime implementing those rules, and their existing native extended tests still pass.

## Independent verification

All commands completed with exit 0 during this review:

| Check | Observed result |
|---|---|
| `python3 scripts/check_vm_extended_opcode_profile.py` | PASS, revision 15, eight implemented slices, status `S20_260_270_EXTENDED_IMPLEMENTED_REVIEW_PENDING` |
| `python3 -m unittest bench.review.tests.test_vm_extended_evidence -v` | 4 passed |
| `cargo test -p sley-vm extended_tests --lib` | 29 passed, 0 failed, 1 ignored fixture emitter, 63 filtered out |
| `python3 scripts/generate_vm_extended_fixtures.py --check` | PASS, no drift, 31 vectors; this separately runs the normally ignored native emitter |
| `uv run --project oracle/scb1 --frozen --offline sley2-scb1-oracle check-vm-extended --accepted conformance/vm-extended/v1/accepted.json --rejected conformance/vm-extended/v1/rejected.json` | PASS, 31 accepted and 5 rejected vectors; explicitly container/cache identity only, no semantic judgment |
| Independent JSON comparison against `git show HEAD:conformance/vm-extended/v1/accepted.json` | Every field of all 29 earlier vector records is unchanged; current corpus has exactly two additional records |

### New frozen outcomes

- `call-direct-depth-exceeded`: `ResourceLimit(CallDepth)`, tag 5; `fuel_used=511`; `instruction_count=0`; observation `74d4ec126dbc926ea1d95f0b2b34873f53d69fcd240630ed5001f11c51f866fd`; no success hash.
- `map-new-duplicate-key`: successful VM execution with `Err(DuplicateKey,1)`; result hash `64f6bbd05a4cade5cebe884435e0d2a92f92986e5e5d5de3d1ab30f526c8c679`; observation `754721f02444bddd213b5fed0675b5d27bf37f96b4cf8e403bc2e34ba21e5d9b`.

## Issues and limits

No new issue requiring a correction was identified within this bounded review. Native tests/emitter establish the added outcome semantics; the Python oracle does not independently execute them. Broader performance, E8 architecture, multi-host behavior, release qualification and unrelated review obligations were outside this task.

The historical transcript and original matrix should remain preserved. The original Nabu headline `PASS_0_P0_7_P1_6_P2_5_P3` must not be rewritten as if it originally contained zero findings; this report supplies later correction evidence for those IDs.

No repository source changes were made by this reviewer. Only this review artifact was written.

## Review input fingerprints

```text
282b622ca511426ba528fa222c65ecc583ce99ebdc5fdc47ee183665dfe0f398  docs/spec/VM_EXTENDED_OPCODE_PROFILE_V1.md
5527a5f4bfc95d78ef2950c38e278796dd9d7cf4b368d00a50d269d3e8f6ea83  scripts/check_vm_extended_opcode_profile.py
e70994da02955d0e2ba866584f62c55b6bf5cb3277ace142929ec43bb6397b83  crates/sley-vm/src/extended_tests.rs
bf9ca182c72c0b13f2cccd7c67a0aef2bbed65d2874ec50e1bfacdf1063a81aa  conformance/vm-extended/v1/accepted.json
4d7393d3fdb2c059dffdaa90a810f4be204b2a820bcd3765d8a0fe2cabaf74ad  bench/review/tests/test_vm_extended_evidence.py
```

## Post-review record sanity check — 2026-09-15

The subsequent summary/spec/ADR/campaign record transitions are consistent
with this review's limited scope. The original Nabu disposition and transcript
remain preserved; the new base disposition is expressly attributed to later
independent item-level closure, not a newly issued Nabu verdict. COMPLETE and
`implementation_complete=true` describe the existing implemented profile with
these historical residuals corrected; the accompanying notes retain the E7
exclusions and make no full-VM or release-acceptance claim. The earlier
references in this report to “pending” describe the pre-transition state.

E4 and E6 each have exactly four matching corpus records. The stage checker
passes at revision 15 / COMPLETE. The four non-document fingerprints above
are unchanged; the spec's updated review-status wording now hashes to
`1dddf3e2e0071221ddc0ab799d809e859960810f8af7b001208d69b9a3046dd4`.
The tracked and checkpoint copies were identical before this note and both
received the same append. No further source changes or broader approvals
were made by this sanity check.
