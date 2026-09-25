# RW-075 correction: genuinely callable `RAW_BLAKE3_V1` via successor (AR-02 close)

Status: IMPLEMENTED (code + successor records + fixtures green; Nabu
round-7 REQUESTED; premium delta REQUESTED; RW-080 remains BLOCKED).
`rw-075.md`, `rw-075-premium-fail.md`, `rw-075-hash-inventory.md`,
`BOOTSTRAP_PROFILE_1` v1, `HOST_ABI_V1` v1, `EXEC_PACKAGE_V1` v1,
`RAW_HASH_V1` v1, RW-050/RW-060/RW-070 evidence, historical
`BOOTSTRAP_READY TRUE`, and `R2_ARCHITECTURE_FAIL` are preserved
byte-identical as history. This file is the bounded correction delta.

## 0. AR-02 was still OPEN (verified before editing)

Premium `R2_ARCHITECTURE_FAIL` AR-02 (BLOCKER) required the self-hosted
compiler's necessary identity/hash operations to be available through an
exact CALLABLE primitive contract. Verified in-tree before any edit:

- `rw-075-premium-fail.md` AR-02: no narrow raw primitive over Sley-built
  preimages existed; any native semantic digest service would conceal
  compiler semantics.
- `conformance/raw-hash/v1/raw-hash.json` admission: explicitly
  `admitted-not-yet-reachable`; `BOOTSTRAP_PROFILE_1 NOT broadened`.
- `conformance/bootstrap-profile/v1/profile.json` permitted imports: 3
  rows (`B2V1`/`V2B1`/`PSH1`); no `RHW1`.
- `crates/sley-vm/src/raw_hash.rs`: pure `raw_blake3_256` existed with
  vectors, but no `adapter_invoke` wiring (`extended.rs` resolver,
  `host_abi.rs` codes, gate admission all 3-row).
- `BOOTSTRAP_PROFILE_1` therefore could not invoke `RAW_BLAKE3_V1`.

Disposition: AR-02 OPEN. This correction makes it callable without
broadening anything else.

## 1. Frozen preservation + successor identities

Preserved byte-identical (verified by v1 checkers):

- `BOOTSTRAP_PROFILE_1` v1 `4f269150...efd630`;
- `HOST_ABI_V1` v1 `e6de00b8...04ecc2`;
- `EXEC_PACKAGE_V1` v1 `9e20da24...595d4`;
- `RAW_BLAKE3_V1` v1 `785205fb...69f72` (primitive unchanged);
- `host-boundary.json` `d935d238...73bdb18a`;
- RW-050/RW-060/RW-070 evidence, historical `BOOTSTRAP_READY TRUE`,
  `R2_ARCHITECTURE_FAIL`, all RW-075 records and review rounds.

Successors (new files, new digests, explicit supersession):

- `BOOTSTRAP_PROFILE_2` v2 (`sley2-bootstrap-profile-2`)
  `fb2d8cc87ee7de68cde8197a77003a417a0062acb6ed087d85f899da1a847459`
  (`conformance/bootstrap-profile/v2/profile.json`,
  `docs/spec/BOOTSTRAP_PROFILE_2.md`,
  `scripts/check_bootstrap_profile_2.py`).
- `HOST_ABI_V2` v2 (`sley2-host-abi-2`)
  `bc564653302a73eb5f998427250a2bb7cd87f5685ef12619bd4ae1f1b2af70d5`
  (`conformance/host-abi/v2/host-abi.json`,
  `docs/spec/HOST_ABI_V2.md`, `scripts/check_host_abi_v2.py`).
- `EXEC_PACKAGE_V2` v2 (`sley2-exec-package-2`)
  `f4958c5e3d57762173b881288b008af17d45b5f07a431fcc442d9eec5770da94`
  (`conformance/exec-package/v2/exec-package.json`,
  `docs/spec/EXEC_PACKAGE_V2.md`,
  `scripts/check_exec_package_v2.py`).
- `RAW_BLAKE3_V1` primitive identity/version/types/vectors unchanged and
  still frozen at `785205fb...69f72`; successor profile/ABI make it
  reachable (no `RAW_HASH_V2` needed; no silent v1 reuse).

Supersession: v1 → retained historical evidence; v2 → current R2
candidate. V2 is a strict superset adding exactly one import row; no
opcode/type/effect/capability broadening. Dependency-closed (same
VM/lowering/epoch/root/cache profile plus the already-pinned
`blake3 =1.8.2`; no new crate/network/toolchain/FFI).

## 2. Exact callable shape

`RHW1` (`raw-blake3-256`):

- Identity `SLY1/BRIDGE/RHW1` zero-padded
  (`534c59312f4252494447452f5248573100000000000000000000000000000000`),
  equal adapter identity, `abi_version` 1, empty effects.
- Operands `(Unit, Bytes)`; result `Result<Bytes, BuiltinFailure(Index)>`
  (the canonical bridge equivalent of `Bytes -> Result<Bytes32, typed
  failure>`; the type carries no length, execution enforces 32 bytes).
- Input 0..=1_048_576 bytes (frozen 1 MiB bridge ceiling); output exact
  32 bytes; over-bound refuses as typed `Err(Index, 2)` (never
  truncation); larger preimages chunk via repeated calls under the driver.
- Fuel `1 + ceil(len/1024)` (`raw_hash_fuel`), charged up front through
  `charge_action` (starved budgets terminate without hashing).
- Deterministic errors: `ResourceLimit` past the bound (as a typed value
  on the bridge; as `RawHashError::ResourceLimit` on the pure call),
  `UnknownVariant` for any algorithm tag other than 1 (pure call;
  unknown bridge identities/versions deny with
  `VM_LOWER_OPCODE_UNSUPPORTED`).
- Algorithm BLAKE3-256, audited `blake3 =1.8.2` (pin already carried by
  `sley-id`); standard vectors empty `af1349...3262`, `abc`
  `6437b3...2fc3`; boundaries 0/1/1024/1 MiB admit, 1 MiB+1 refuses;
  tamper/determinism/no-domain pins.
- Forbidden semantic services (`fingerprint(program)`,
  `object_id(program)`, `validate_and_hash_object`,
  `candidate_digest(high_level)`, any program/candidate/type/inventory →
  digest/verdict service) remain absent (marker + behavioral probes).
  Unknown hash identities/versions fail closed. No general FFI.

## 3. Successor bootstrap binding

R2 bootstrap environment positively admits exactly four operations
(`B2V1`/`V2B1`/`PSH1`/`RHW1`) with default denial retained. Profile/ABI/
package/cache/admission updated consistently:

- profile/ABI v2 records (4 rows, v2 digests, supersession fields);
- package v2 (envelope 2, profile digest v2, ABI version 2; same
  layout/bounds/sections/hydration/`SLEYPOBS1` domain; v1 envelopes still
  verify under v1 functions);
- cache profile unchanged (`EXTENDED_V1`, same twelve preimage fields);
- admission (`BootstrapProfileReport` sealed, entry-first/import-set/
  counts/fingerprints/image-digest consistency) unchanged in discipline,
  now covering `RHW1` closures through the one shared resolver.

## 4. Invalidated closure revalidated (only that)

V1 files preserved; live successor revalidated:

- `check_bootstrap_profile_1.py` PASS (v1 preserved) +
  `check_bootstrap_profile_2.py` PASS (successor);
- `check_host_abi_v1.py` PASS + `check_host_abi_v2.py` PASS +
  `check_host_abi_markers.py` PASS (now pins `RHW1`/V2);
- `check_exec_package_v1.py` PASS + `check_exec_package_v2.py` PASS +
  `check_exec_package_markers.py` PASS (now pins v2);
- `cargo test -p sley-vm`: all suites green including new
  `rw075_raw_callable` (13 tests: callability, vectors, boundaries,
  tamper, fuel/pre-charge, unknown/wrong-version negatives, v2 package
  binding + v1/v2 mismatch negatives, staged authority
  graphs-to-image binding + tamper refusal, graph-A/B substitution
  refusal, large-preimage composition, 11-domain preimage ownership +
  order negative, Sley-built four-step executed assembly, SLEYBC02
  boundary);
- v1 20 closure vectors replay unchanged under the successor gate
  (superset; same `accepted.json` bytes in v2);
- RW-060 lifecycle preserved as history; its 15 driver tests remain green
  in-tree (no silent re-attribution: successor package tests bind v2
  digests explicitly);
- real image-emission + mixed compiler-like workloads re-run through the
  successor boundary in the new fixtures (package-path `RHW1` execution
  + reference comparison);
- aggregate R2 gate inputs updated to consume successor P/H/package
  identities (see §7).

No unaffected campaign history rerun.

## 5. Semantic preimage ownership

`crates/sley-vm/tests/rw075_raw_callable.rs`
`raw_preimage_ownership_across_compiler_domains`: for each representative
domain (`SLEYSFP1`, `SLEYVHS1`, `sley2.value-hash.v1`,
`sley2.semantic-fingerprint.v1`, `sley2.vm-bytecode-cache-key.v1`,
`sley2.observation.v1`, `sley2.entity/object/state-root/transaction/
candidate.v1`) Sley-built preimage bytes → `RHW1` → expected canonical
digest, compared with the independent `blake3` reference. Negatives prove
changing domain separator / field bytes / order changes the identity.
The native primitive never sees a program/candidate/type/inventory and
knows no domain.

## 6. SLEYBC02 encoding reachability disposition

Native structural parsing/loading of an already-created `SLEYBC02` image
(`check_image_prefix`, `load_image`) is permitted and remains. High-level
compiler image construction/lowering (`encode_sleybc02(semantic_program
or instruction_graph) -> image` performing layout/assembly decisions)
is Sley-owned and NOT reachable: no bridge identity encodes semantic
programs (probe denies `ENC1`/`IMG1`/`CMP1`/`LOW1`/`BLD1` spellings);
bridge/host-abi sources carry no `encode_sleybc02`/`construct_compiler_
image`/`assemble_ssmc`/lowering/fingerprint services (source probe);
`lower::encode_function` is seed-reference outside the clean closure.
If a low-level byte/framing encoder remains native (package/image section
codecs), it is mechanical (length-prefixed framing, depth/byte ceilings,
no layout/order/tag decisions); Sley supplies all compiler-owned
layout/order/tag decisions via checker/lowerer/builder (RW-080 contract).
No widening performed; no violation found.

## 7. Review order + R2 gate

- Nabu round-7 delta re-review: returned FAIL with two BLOCKERs,
  preserved unedited in
  `reviews/reweave-rw075-nabu-r7-2026-09-06.log` (commit `9574f8a`
  reviewed). Repaired below as a separate delta (repair-only, no
  widening); round-8 re-review requested.
- If Nabu FAILS: repair only the finding, repeat Nabu. If PASSES:
  request premium DELTA-ONLY re-review from the same role that issued
  `R2_ARCHITECTURE_FAIL` (exact `VERDICT: R2_ARCHITECTURE_PASS` consumed
  by the gate; no self-certification; no asking premium to overlook
  findings because package checks pass).
- Fresh `BOOTSTRAP_READY` only after Nabu final PASS + premium
  `R2_ARCHITECTURE_PASS` + aggregate R2 gate READY on successor
  identities. S/C0/C1/C2/C3 and RW-080+ remain later-stage. Stop before
  RW-080.

Current verdicts: Nabu round-7: FAIL (two BLOCKERs, preserved); Nabu
round-8: FAIL (R7-B2 hardening + executed-assembly BLOCKERs, preserved);
Nabu round-9: FAIL (authority-divergence + exclusivity + assembly
BLOCKERs, preserved); Nabu round-10: FAIL (receipt-forgeability +
executed-tamper BLOCKERs, preserved); Nabu round-11: PASS (final, no
blockers, preserved in
`reviews/reweave-rw075-nabu-r11-2026-09-06.log`); premium delta:
FAIL (`R2_ARCHITECTURE_FAIL`, preserved in
`reviews/reweave-rw075-premium-r1-2026-09-06.log` at `6c3d5df`: AR-01
CLOSED; AR-02 through AR-06 PARTIALLY_CLOSED; new AR-07 BLOCKER (v2
admission requires native semantic judgment/lowering per package) and
new AR-08 HIGH (v1 authority not isolated from successor imports);
gate-script weaknesses noted for `premium_verdict` supersession,
`premium_verdict` prefix parsing, and R2 evidence binding);
aggregate R2: NOT_READY (Nabu complete, premium FAIL — honest, not a
defect in the repair).

## 8. Round-7 repairs (this delta, reviewable in round 8)

- R7-B1 (1 MiB vs larger preimages): replaced the hand-wavy "chunked by
  the driver" note with the exact frozen `SLEYCHNK1` composition contract
  (`BOOTSTRAP_PROFILE_2.md`, `HOST_ABI_V2.md`): one-shot primitive stays;
  over-bound preimages split into 1 MiB chunks hashed via `RHW1`, framed
  with `SLEYCHNK1 || u32(1) || u32(N) || chunk digests` built by Sley,
  final hash via `RHW1`. All R2 fixture preimages measure <1 KiB
  (single-shot); 1 MiB+1 and 2 MiB composition vectors prove the rule
  through the callable (`raw_large_preimage_composition_is_exact`). No
  primitive change, no new import, no streaming state.
- R7-B2 (graphs-to-image correspondence): added the staged v2 admission
  authority as explicit code (`stage_v2_admission` in
  `rw075_raw_callable.rs`): judge the closure, reference re-lower with
  the native lowerer, compare bytes exactly, verify gate claims, mint a
  receipt only on exact match (plus approval cross-check, so no
  unapprovable receipt is ever returned). Honest packages admit;
  single-byte-rewired images refuse with no receipt
  (`raw_staged_authority_binds_graphs_to_image`). Production
  `admit_package_v2` stays pure-data (as in v1); R2 evidence uses only
  authority-minted receipts (CI), and the Sley build driver replicates
  the same comparison per the RW-080 contract §1.4. No toolchain graph,
  no C1, no RW-080 construction in this repair.
- R7 observation (label-string preimages): added the end-to-end
  Sley-built assembly fixture (`raw_sley_built_preimage_end_to_end`): a
  four-step Sley function (`B2V1` + `PSH1` + `V2B1` + `RHW1`) gate-admits
  with four bridge uses, proving Sley assembles preimage bytes with
  bootstrap ops and the host only hashes; data-plane digest matches the
  reference. Full `Result`-threading across fallible bridge calls stays
  with RW-110 (documented in-fixture).

## 9. Round-8 repairs (this delta, reviewable in round 9)

- R8-B1 (production authority, exclusive minting): promoted the staged
  authority from a test helper to the production module
  `sley_vm::admission_authority` (`admit_v2_package` over one canonical
  `V2Closure` bundle from which gate and lowering inputs derive
  internally, so graph-A/gate versus graph-B/lowering cannot diverge;
  `AuthorityError` vocabulary including `UnknownEntry`; unit pins). The
  raw v2 constructor is `pub(crate)` with its public re-export removed,
  so reviewed integration paths mint exclusively through the authority;
  direct constructor calls exist only in crate unit tests as explicitly
  marked negatives. Graph-A(RHW1)/image-B(B2V1) adversarial regression
  (`raw_authority_refuses_graph_a_gate_with_graph_b_image`) plus the
  single-byte tamper refusal prove no receipt on substitution. Marker
  pins added (`check_exec_package_markers.py`); the package/execution/
  raw-hash/host-ABI/bridge modules stay free of gate/lowerer calls.
- R8-B2 (executed assembly): `raw_sley_built_preimage_end_to_end` is a
  well-typed, lowered, executed four-step Sley path over separate domain
  (`Bytes`) and field (`UInt(8)`) inputs: `B2V1` converts, `VariantSwitch`
  unwraps, `PSH1` joins the field, `VariantSwitch` unwraps, `V2B1`
  converts back, `VariantSwitch` unwraps, `RHW1` hashes the Sley-assembled
  bytes and returns the digest directly (`Err` legs wrap and return).
  Gate admits four bridge uses; lowering and execution run through the
  successor registry; the digest matches the reference over the joined
  bytes and either input owns the identity.
- R7-B1 typo: `SLEYCHNK1` frame is `17 + 32*N` bytes (9-byte domain +
  two `u32` words + digests), corrected in `BOOTSTRAP_PROFILE_2.md`.

## 10. Round-10 repairs (this delta, reviewable in round 11)

- R10-B1 (receipt unforgeability): `AdmissionReceipt` is sealed
  (`#[non_exhaustive]` plus private fields) with `package_digest` /
  `profile_digest` / `host_abi_version` const accessors, so no downstream
  struct literal or mutation can forge one (v1 call sites unchanged in
  behavior: `admit_package` still mints v1 receipts through the same
  constructor shape, now sealed). The raw v2 constructor stays
  `pub(crate)` with no public re-export, and the marker now pins the seal
  plus a production-source exclusivity scan (only `exec_package.rs` and
  `admission_authority.rs` may name the minter). Residual in-crate
  test-negative construction is explicitly marked non-evidence in-module.
- R10-B2 (executed tamper): both altered cases now execute altered inputs
  through Sley (`run_assembly(b"SLEYSFP1", 0x42)` and
  `run_assembly(b"SLEYSFP2", 0x41)`), each matching its own reference and
  differing from the honest digest.

## 11. Round-12 plan (premium FAIL repairs, verified inputs only)

Premium delta `reviews/reweave-rw075-premium-r1-2026-09-06.log` at
`6c3d5df` holds the repair order. Verified in-tree inputs (no fix yet):

- AR-02: canonical identity is single-shot `BLAKE3(domain || preimage)`
  (`sley-id/src/lib.rs` `digest`); `MAX_FINGERPRINT_PREIMAGE_BYTES` is
  67_108_864 (`sley-ssmc/src/fingerprint.rs:21`) against
  `RAW_HASH_MAX_BYTES` 1_048_576 (`sley-vm/src/raw_hash.rs:72`), so a
  compiler preimage may legally exceed the primitive bound 64x and the
  `SLEYCHNK1` hash-of-chunks construction cannot carry canonical
  identity (premium calculation: full `b2feb6...4f54e9` vs chunked
  `0bc370...4582`, unequal). Repair direction: enforceable 1 MiB
  bootstrap-profile bound plus measured evidence that every
  bootstrap-required preimage fits with margin plus the existing typed
  over-bound refusal (`Err(Index, 2)`); retire `SLEYCHNK1` as a
  canonical construction (typed refusal replaces it; streaming stays an
  explicit future gap, never a silent redefinition). The unbound
  amendment (prose rule without JSON digest change) is resolved by the
  removal: no normative rule lives in prose alone afterward.
- AR-03: `ExecutionPackage` already carries full inventories
  (`exec_package.rs:210`: constants, type_definitions, exact-row
  imports, globals, contracts, entry, epoch, root). The authority
  (`admission_authority.rs:122`) compares only image bytes plus gate
  counts/fingerprints. Repair: compare package.constants against
  closure.constants, package.type_definitions against closure.types,
  package full import rows against closure.adapters, package
  globals/contracts/entry/epoch/root against the closure, before
  minting; no receipt on any mismatch.
- AR-07: `admit_v2_package` unconditionally calls
  `judge_bootstrap_profile` plus `lower_function`
  (`admission_authority.rs:131/:144`); the RW-080 contract §1.4
  repeats the reference-lowering procedure for the driver. Repair:
  split structural verification (digest equality, binding checks —
  permanent native mechanics, home of the AR-03 comparisons) from
  semantic judgment (native seed path now, declared as C0 seed; typed
  Sley-evidence ingress reserved for post-C1 with no minting path
  until C1 exists); confine native re-lowering to C0/oracle evidence
  in the contract.
- AR-08: one shared `judge_bootstrap_profile` with no version
  selector (`bootstrap.rs:283`; input struct `bootstrap.rs:73` carries
  no profile version) serves both v1 and v2 through one
  `resolve_bridge_entry` that admits `RHW1` (`extended.rs:304`).
  Repair: version selector on the gate input (v1 admits exactly
  B2V1/V2B1/PSH1, v2 admits plus RHW1); execution paths bound to
  admitted packages, direct-execution closures explicitly
  reference-only legacy boundary excluded from clean execution.
- AR-06: `premium_verdict` (`check_r2_exit.py`) returns on the first
  decisive file in sorted order, so an early FAIL masks a later PASS
  (and prefix parsing is not exact full-line). Repair: latest-file
  round semantics mirroring `latest_lane_verdict`, exact full-line
  verdict parsing, review evidence bound to the reviewed
  implementation revision.
- AR-04: unify `rw-080-contract.md` on the successor digests
  (profile v2 `fb2d8cc8...`, ABI v2 `bc564653...`, package v2
  `f4958c5e...`; raw hash reachable, not "awaiting wiring"),
  byte-level module and admission handoffs, chunk/framing ownership,
  reserved verifier interface, Sley admission-evidence ingress.
- AR-05: replay branching-traversal plus image-construction workloads
  through the successor (v2) runner with Sley-side/host-side
  attribution stated per metric; no host-owned algorithm reported as
  Sley-owned.

Order: AR-02 measurement first (it constrains the primitive every
other repair builds on), then AR-03 + AR-08 + AR-06 (bounded code),
then AR-07 + AR-04 (contract/design), then AR-05 evidence, then Nabu
round-12 re-review and a fresh premium delta re-review. RW-080 stays
BLOCKED throughout.

AR-02 probe done: `rw-075-ar02-evidence.md`. Cap-the-bound is viable
(all real Sley-side RHW1 preimages measure <= ~1 KiB; all large
digests are native single-shot and never route through RHW1); repair
is (i) v2 admission bound on carried `Bytes` (rides AR-08 selector)
plus the existing typed execution refusal as backstop, (ii) retire
`SLEYCHNK1`, (iii) streaming deferred as an explicit gap, (iv)
restate the ungrounded "<1 KiB adopted fixtures" claim as a measured
suite property (AR-04).

## 12. Round-12 implementation (this delta, reviewable in round 12)

- AR-06 (`scripts/check_r2_exit.py`): `premium_verdict` now mirrors
  `latest_lane_verdict` (latest round file only, `*infra*` excluded)
  with token-exact verdict parsing (a longer token never parses;
  trailing reviewer notes still parse). Verified with scratch r2 files
  (removed after): later PASS supersedes r1 FAIL, `PASS_EXTRA`
  parses PENDING (still blocks), within-file FAIL keeps precedence.
  Gate still reads NOT_READY on the retained r1 FAIL.
- AR-08 (`sley-vm/src/bootstrap.rs`, `exec_package.rs`,
  `admission_authority.rs`, `bootstrap_closure.rs`, all gate-input
  call sites): new `BootstrapProfileVersion` (`V1`/`V2`) on the gate
  input, recorded in the report (new `profile_version` accessor);
  `RHW1` denies under V1 in `bootstrap_row_ok`; v1 approval requires
  a V1 report and v2 approval a V2 report (closes cross-version
  report replay — a V2 report naming `RHW1` can never back a v1
  package). Negatives: `raw_v1_gate_refuses_successor_row`,
  `raw_v1_approval_refuses_successor_report`,
  `raw_v2_approval_refuses_v1_report`.
- AR-02 (gate + specs + tests): v2 admission refuses carried `Bytes`
  over 1 MiB with `ResourceLimit` (V1 needs no bound: no `RHW1`,
  `ValueHash` stays canonical under its own limits); `SLEYCHNK1`
  retired (`BOOTSTRAP_PROFILE_2.md` rule replaced, `HOST_ABI_V2.md`
  note replaced, composition test replaced by
  `raw_over_bound_refuses_without_composition`, which pins refusal
  plus framing-domain inertia). Negative:
  `raw_v2_gate_bounds_carried_preimages` (over-bound refuses,
  exactly-1 MiB admits, V1 admits the same constant). Evidence:
  `rw-075-ar02-evidence.md`.
- AR-03 (`sley-vm/src/admission_authority.rs`): the authority
  compares entry/epoch/root, constants, type definitions (by identity
  through the environment), full import rows (order-insensitive, not
  only the identity set), globals, and contracts against the judged
  closure before minting; any divergence refuses `ClaimsMismatch`
  (variant doc broadened). Negative:
  `raw_authority_refuses_substituted_tables` (constants, import row,
  global, epoch legs).
- Validation (Tier 2): sley-vm full suite 176 passed, 0 failed
  (83 lib + 37 freeze + 30 exec_closure + 8 hydration + 18
  raw_callable); sley-repo rw060 15 passed; profile/ABI freeze
  checkers plus exec-package markers PASS; `cargo fmt --check`
  clean; clippy 0 warnings (sley-vm, sley-repo); workspace
  `--all-targets` check clean.
- Still pending: AR-07 + AR-04 (contract/design), AR-05 evidence,
  Nabu round-12 re-review, then a fresh premium delta re-review.
  RW-080 stays BLOCKED; the R2 gate stays NOT_READY until a new
  premium round passes.

## 13. Round-12 part 2 (this delta: AR-07 + AR-04 + AR-05)

- AR-07 (`sley-vm/src/admission_authority.rs`, `lib.rs`): the staged
  authority is split into declared C0-seed semantic legs
  (`judge_closure_for_seed`, `reference_lower_for_seed` — native gate
  plus reference lowerer, confined to C0 and excluded from clean
  stages) and permanent structural mechanics
  (`verify_structural_correspondence` — byte equality, claim binding,
  complete table correspondence, image-digest binding — plus digests,
  minting, approval cross-check, all judgment-free). Reserved
  post-C1 ingress: sealed `SleyAdmissionEvidence` (no constructor
  until C1) plus `admit_v2_package_from_sley_evidence`, which always
  refuses `SleyEvidenceUnavailable` — a typed reservation with no
  minting path. Negatives: stable error codes incl. the new variant;
  ingress-has-no-minting-path pin. Marker script still PASS.
- AR-04 (`rw-080-contract.md`): unified on the successor baseline
  (profile v2 `fb2d8cc8...`, ABI v2 `bc564653...`, package v2
  `f4958c5e...`; `RAW_BLAKE3_V1` admitted and reachable, not
  "awaiting wiring"; v1 records stay byte-identical history, no
  module builds under them). §1.4 stages builder faithfulness
  (C0 native seed now, Sley-owned evidence post-C1 via the reserved
  ingress; no hidden fallback/image/replay in clean stages). New
  §1.6 byte-level handoffs (codec/checker/lowerer/admission shapes
  as exact bytes plus digests), retired-chunk rule (over-bound
  refuses typed; `SLEYCHNK1` reserved-unused; streaming needs its own
  domain), host-vs-staging split (registry exactly
  B2V1/V2B1/PSH1/RHW1; recorders are external staging), and a
  canonical reserved verifier interface
  (`verify_witness` + `VerifierResult`/`VerifierError`, RW-150/170).
- AR-05 (honest workload evidence): new in-crate
  `closure_workloads_replay_through_v2_with_attribution` replays the
  Sley-owned traversal/emission workloads (worklist DFS chain +
  cycle, image-assemble-emit, value-hash-chain,
  checked-length-traverse) through admit/approve/execute v2,
  asserting v2 termination equals direct execution plus expected
  values per case, with per-metric attribution (Rust seed vs Sley
  execution vs harness). Measured: chain 2204 B / 34 instr / 124
  fuel; cycle 2204 B / 23 / 81; emit 938 B / 19 / 75;
  value-hash 304 B / 4 / 5; traverse 300 B / 2 / 12.
  `rw075_hydration_workloads.rs` narrowed to stated ownership
  (Rust-driven steps; reference emission; sequenced packages;
  `transport_bytes` replaces synthetic `copied_bytes`; one counted
  execution per edge).
- Validation (Tier 2): sley-vm full suite 178 passed, 0 failed
  (85 lib + 37 freeze + 30 exec_closure + 8 hydration + 18
  raw_callable); sley-repo rw060 15 passed; profile/ABI freeze
  checkers plus exec-package markers PASS; `cargo fmt --check`
  clean; clippy 0 warnings (sley-vm, sley-repo); workspace
  `--all-targets` check clean; `git diff --check` clean.
- Still pending: Nabu round-12 re-review, then a fresh premium delta
  re-review. RW-080 stays BLOCKED; the R2 gate stays NOT_READY until
  a new premium round passes.
