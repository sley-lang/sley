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
executed-tamper BLOCKERs, preserved in
`reviews/reweave-rw075-nabu-r10-2026-09-06.log`); Nabu final: PENDING
(round-11 requested); premium delta: PENDING; aggregate R2: NOT_READY
(implementation complete, reviews pending — honest, not a defect in the
repair).

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
