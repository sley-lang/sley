# RW-080 §1.1 program slice 5: EntryPoint kind-16 Sley encode emission — provisional C0 construction record

Status: PROVISIONAL (2026-09-07, operator development override
`rw075_correction.operator_override_2026_09_07`, continued per operator
instruction "Continue RW-080 slice 5"). One Sley function with a real
strict codec algorithm through the v2 admit/approve/execute boundary
(`encode_entrypoint`), completing the parked slice-4 stub. Not accepted
runtime authority; `rw080` stays BLOCKED and the R2 gate stays NOT_READY
pending independent Nabu round-12 review plus premium round 2 plus S20-780
independent acceptance. Prior slices retained unchanged
(`rw-080-codec-uvar.md`, `rw-080-codec-envelope.md`,
`rw-080-codec-program-outer.md`, `rw-080-codec-program-body.md`); this
record covers only the slice-5 delta. No amendment duplication, no
reconciled settlement reopened, no production/gate/ledger-status change.

## 0. Scope (no new planning campaign)

Slice-4 §9 next bounded dependency: "entrypoint encode emission (fixed
pushes + 32B copy + V2B1 ...) then program envelope validate/build in Sley
..., then composed entries with single-invocation measurement." This slice
does the first item only. RW-080 construction stage and contract
(`rw-080-contract.md`); RW-090 codec parity gate; RW-100 semantic gate only
(never imported here).

## 1. What was built (seed-authored Sley, this file is not the assembler; source is `crates/sley-vm/tests/rw080_codec_program_outer.rs`)

- `encode_entrypoint(function: Bytes, exposure: UInt64, unit: Unit) -> Result<Bytes, Bytes>`
  canonical 40B emission for closed kind 16 (`1026...`): exposure pinned
  1/2 first (`SCB_UNION_INVALID` otherwise, validated before function
  work — documented order, both value errors); function B2V1 then
  `VectorLen` with `<32 SCB_LENGTH_OVERFLOW` / `>32 SCB_TRAILING_BYTES`
  (encode-side mirror of `decode_fixed`, same as outer-encode eid check);
  fixed pushes `10/26/02/01/20` (union tag 16, union len 38, count 2,
  field-1 tag 1, field-1 len 32); 32B function copy loop (Form A
  backedge, mirrors outer eid loop, `VectorGet` + PSH1 + `IntAddChecked`,
  Get-None unreachable after the ==32 check, increment-overflow to trap);
  field-2 tag `02` + len `01`;   exposure byte via 1/2 select pushing `u01`/
  `u02` constants (no u64-to-u8 conversion opcode needed); V2B1 finalize;
  `ResultOk`. Canonical wire layout, 5 + 32 + 3 = 40 bytes total:
  `10 26 02 01 20 <32 function bytes> 02 01 <01|02>` (union tag 16,
  union len 38, count 2, field-1 tag 1 len 32, function body, field-2
  tag 2 len 1, exposure byte). Bridge uses B2V1/PSH1/V2B1 only. No uvar callee (all
  single-byte for this fixed shape). No loops beyond the bounded 32B copy.
- Correction to the slice-4 park note: the parked comment claimed the
  `entry` true-branch (exp==1 straight to `func_conv`) had a target-arity
  bug requiring an exposure-drop adapter block. Re-examination shows all
  pushed edges were already arity-correct (3 args to 3 params on both
  paths; the confused comment thread is removed). The actual park was the
  `func_conv` Ok arm halting at `b_res` (RESOURCE) instead of continuing
  to the length check. This slice repoints Ok to `func_len` and completes
  the pipeline; no adapter block was needed. Verified empirically: the
  completed image admits and executes, it was not approved from the
  comment's reasoning.
- Tests (same file): `entrypoint_encode_image` (single-function image,
  entry `(9,37)`, Ns 101-104, B2V1/PSH1/V2B1 frozen imports),
  `entrypoint_encode_call`, `entrypoint_encode_probe_roundtrip`
  (encode 0a*32/1 and 0b*32/2 byte-exact against the slice-4 decode
  vectors, determinism double-run, decode-back through Sley decode),
  `entrypoint_encode_rejections` (exp 0/3/19 UNION_INVALID; 31B function
  LENGTH_OVERFLOW; 33B function TRAILING_BYTES).

Images: entrypoint-encode (entry + no callee). Admitted under
`BOOTSTRAP_PROFILE_2` through `admit_v2_package` (C0 seed route,
declared). Entry namespace `(9,37)` under disjoint Ns 101-104,
deterministic and collision-free by construction (prior images use
(9,21-23), (9,31-36)).

## 2. Measurements

- `ENTRY_ENC_LOCAL` (40B): fuel 1645, instr 223, peak 29967.
- `ENTRY_ENC_PROTO` (40B): fuel 1655, instr 226, peak 30013.
- Far inside the 100K/1M/1M codec limits; value-units bind first, same as
  prior slices. No new budget claim.

## 3. Contract decisions (explicit, reviewable)

- Error-code preservation: refusals carry the exact `SCB_*` strings;
  exposure validated before function length (both value errors, order
  documented, no reference precedence to mirror on encode).
- Parity basis: byte-exact match against the two slice-4 Sley-proven
  vectors plus Sley-decode roundtrip. No native-encode oracle call exists
  in this test file (parity class matches slice-4 hand-derived vectors);
  RW-090 remains the byte/error parity gate against `sley-scb1` native
  plus the scb1 oracle.
- Opaque scope unchanged: outer `Ok` still does not imply body validity;
  entrypoint encode emits the closed kind-16 shape only.

## 4. Explicit non-goals (not silent gaps)

- `codec_main` legs stay stubs; no leg wiring.
- Program envelope validate/build in Sley: next bounded dependency, not
  started (session scope ends at encode emission + record).
- Composed outer→entrypoint entries with single-invocation measurement:
  future work after the envelope slice.
- Namespace Sley codec: retained as outer opaque inputs only.

## 5. Review provenance and acceptance debt

Lint triage (repo zero-warning standard): `cargo fmt --all` clean
(applied, touched only this test file); `cargo clippy --workspace
--all-targets -- -D warnings` clean;
`git diff --check` clean. Author: same session/model as the candidate
(self-review class under the standing amendment; supports provisional
development only, never independent acceptance). Independent native
reviewer: not obtained in this session. No verdict files manufactured; no
gate edits; no promotion to runtime authority. Acceptance debt unchanged
and still required: Nabu round-12 review, premium round-2
`R2_ARCHITECTURE_PASS`, S20-780 independent acceptance. Retained premium
FAIL and `R2_EXIT: NOT_READY` preserved; `rw080` stays BLOCKED. No
fabricated verdict or full-suite claim. Tier 1 / targeted validation per
tiered-validation policy (affected: `sley-vm` program-outer 15/15 — was
13/13, +2 new; uvar 9/9; envelope 9/9; scaffold 2/2; `sley-scb1` lib 6/6;
`sley-mutate` object 5/5; fmt/clippy/diff-check clean). Full gate not run
(not required for this development iteration).

## 6. Identities

- Source: `crates/sley-vm/tests/rw080_codec_program_outer.rs` (seed-assembler
  artifact; 15 tests green); exact starting HEAD
  `699c2283dd73d836630d0b81d01ac78a6aadf6d2` (this slice's parent:
  slice-4 checkpoint plus brand image) through this slice's working tree.
- Graphs/images: admitted per-test at runtime (no checked-in image).
  The test path emits no graph-root hash and no image digest (in-memory
  `Image`, lowered and admitted per test, nothing printed or hashed);
  the binding identity this path produces is the per-test receipt
  assertion `receipt.profile_digest() == BOOTSTRAP_PROFILE_2_DIGEST`
  (`rw080_codec_program_outer.rs:11195-11201`), binding profile
  `BOOTSTRAP_PROFILE_2`
  (`fb2d8cc87ee7de68cde8197a77003a417a0062acb6ed087d85f899da1a847459`
  per `crates/sley-vm/src/exec_package.rs:123-126`);
  entry `encode_entrypoint` `(9,37)` under Ns 101-104.
  Runtime-generated identities are execution evidence (admit receipts
  assert the profile digest per test); "generated and admitted per test"
  is construction, not a substitute for these identities.
- Dependencies: `HOST_ABI_V2` imports B2V1/PSH1/V2B1 only
  (`bc564653302a73eb5f998427250a2bb7cd87f5685ef12619bd4ae1f1b2af70d5`
  per `conformance/host-abi/v2/SHA256SUMS` and `docs/spec/HOST_ABI_V2.md`;
  RHW1 unused in Sley here); `EXEC_PACKAGE_V2` via C0 seed route
  (`f4958c5e3d57762173b881288b008af17d45b5f07a431fcc442d9eec5770da94`
  per `conformance/exec-package/v2/SHA256SUMS` and
  `docs/spec/EXEC_PACKAGE_V2.md`, unchanged by this slice); reference `sley-scb1` (uvar/record/union/encode primitives)
  + `sley-mutate` (`EntityBodyValue::EntryPoint`,
  `SSMC1_EPOCH1_SCHEMA.txt` rows 9/25/28).
- Fixtures: ep-local/ep-proto 40B bodies (func `[0a;32]`/`[0b;32]`,
  exposure 1/2); limits `codec_limits`; commands
  `cargo test -p sley-vm --test rw080_codec_program_outer` (15/15),
  `--test rw080_codec_uvar` (9/9), `--test rw080_codec_envelope` (9/9),
  `--test rw080_codec_scaffold` (2/2), `-p sley-scb1 --locked --lib`
  (6/6), `-p sley-mutate --locked --lib object` (5/5), `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`,
  `git diff --check`.
- Validation tier: Tier 1 / targeted (listed above). Full gate: not run.
