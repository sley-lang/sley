# EXEC_PACKAGE_V2

Status: frozen successor (RW-075 correction, 2026-09-06). Version 2.
Current R2 candidate. `EXEC_PACKAGE_V1` v1 preserved byte-identical as history.

- Package digest record: `f4958c5e3d57762173b881288b008af17d45b5f07a431fcc442d9eec5770da94`
  (raw-byte SHA-256 of `conformance/exec-package/v2/exec-package.json`;
  `scripts/check_exec_package_v2.py` verifies the binding).
- Contract: `sley2-exec-package-2`. Machine record:
  `conformance/exec-package/v2/exec-package.json` (authoritative for values);
  this document (authoritative for rationale and rules).
- Rust surface: `sley_vm::exec_package` (`package_digests_v2`,
  `encode_package_envelope_v2`, `decode_package_envelope_v2`,
  `hydrate_package_envelope_v2`, the four structural section decoders,
  `approve_package_v2`, `verify_package_binding_v2`,
  `BOOTSTRAP_PROFILE_2_DIGEST`, `EXEC_PACKAGE_V2_*`; the raw v2
  constructor is crate-private with no public re-export) plus
  `sley_vm::execute_approved_package_v2` plus the staged authority
  `sley_vm::admission_authority::admit_v2_package` (one canonical
  `V2Closure` bundle, judge plus reference re-lower derived internally,
  mint only on exact match; the exclusive v2 minter in reviewed paths).
  `AdmissionReceipt` is sealed (`non_exhaustive` plus private fields)
  with `package_digest`/`profile_digest`/`host_abi_version` accessors,
  so no downstream literal or mutation can forge one.
- Supersedes: `EXEC_PACKAGE_V1` v1
  (`9e20da24a3b3647d15d052ce759ed9b1ca7682d950421baf48978ad59a5795d4`,
  `conformance/exec-package/v1/exec-package.json`,
  contract `sley2-exec-package-1`).
  Relation: same envelope layout/bounds/sections; version field 2,
  `profile_digest` v2
  (`fb2d8cc87ee7de68cde8197a77003a417a0062acb6ed087d85f899da1a847459`),
  `host_abi_version` 2. V1
  preserved functional for legacy evidence only; new executions use v2.

## What this is

`EXEC_PACKAGE_V2` is the complete execution-package closure binding the
successor profile/ABI. Same envelope as v1 (`SLEYPKG1`, sections
image/constants/layouts/imports/dependency, same ceilings: total
67_108_864 with 8 MiB constants/layouts/dependency and 1 MiB imports
sub-budgets, same counts), same structural hydration
(`hydrate_verified_definitions`: duplicate/bound checks only), same
`SLEYPOBS1` package observation domain (plus every section digest plus
profile digest plus ABI version, so v1 and v2 packages never share
authority evidence). V1 (`BOOTSTRAP_PROFILE_1` digest, host ABI 1,
envelope 1) is history; v2 (profile v2 digest, host ABI 2, envelope 2)
is the current candidate.

It broadens nothing except the successor import row (`RHW1` exact rows
bind through the same full-row digest; unknown rows still refuse).

## Bindings

Package digest preimage (SHA-256):
`SLEYPKG1 || u32(2) || profile_digest(v2) || host_abi_version(2) ||
vm_version || image_digest || constants_digest || layouts_digest ||
imports_digest || dependency_digest || entry || epoch || root`.

Approved binding: package/image/constants/layouts/imports/dependency
digests; exact-row imports; entry/epoch/root/profile (`EXTENDED_V1`);
profile digest v2; VM `[1,0,0]`; host ABI version 2; admitted limits
(request must match exactly); receipt for that exact package digest;
gate report consistency (entry-first, import-set equality,
operation/bridge counts, closure fingerprints, admitted image digest —
verified without running the gate). Cache key re-derived (same twelve
fields). Admitted limits must equal the request limits exactly at
approval (mismatch refuses); observations additionally bind them
post hoc.

## Identity versus serialization

The package identity is the header digest above and nothing else. It is
not a digest over serialized envelope bytes: the sections enter it only
through their section digests. The `EXEC_PACKAGE_V1` wording "package
digest (over the envelope of at most 67_108_864 total bytes)" describes
the same header preimage bounded by the envelope ceiling; that text is
frozen history and is superseded by this statement, not edited.

RW-080 now carries a provisional serialized-envelope candidate alongside the
unchanged package identity. The fixed 316-byte header is exactly the package
digest preimage above. It is followed by five sections in this order: image,
constants, layouts, imports, dependency. Each section is framed as a
big-endian `u64` byte length followed by the exact existing canonical section
bytes. The sum of the five payload lengths remains bounded by
`EXEC_PACKAGE_MAX_BYTES`; the 316-byte header and five 8-byte length fields
are fixed transport overhead.

`encode_package_envelope_v2` emits this layout and
`decode_package_envelope_v2` strictly verifies the fixed profile/ABI/VM
binding, bounds, no trailing bytes, and every section digest. The decoder
returns raw canonical section bytes and performs no semantic judgment or
admission judgment. `decode_constants_section`, `decode_layouts_section`,
`decode_imports_section`, and `decode_dependency_section` hydrate those bytes
structurally with byte/count/depth bounds, strict tags, row framing, and
duplicate-identity refusal. `hydrate_package_envelope_v2` composes the raw and
section decoders, verifies the repeated entry/epoch/root/profile bindings,
requires canonical byte-for-byte re-encoding, and reconstructs the exact
`ExecutionPackage` plus its authenticated digests. It does not resolve
references, judge types/contracts/closure claims, or mint admission evidence.

The candidate vector and independent Python reproduction are under
`conformance/exec-package-envelope/v2/`. The package identity and all existing
section encodings are unchanged. The envelope candidate remains provisional
until the RW-080 contract/surface review accepts it. The bounded Sley
`build_package` entry emits native-identical empty and nonempty
constants/layouts/imports sections plus dependency framing with runtime gate
fingerprints, limits, globals, and contracts, then composes the final header
and five length-prefixed sections. The five SHA-256 values are host-mechanic
execution results under the frozen RW-075 hash inventory and enter distinct
fixed-width digest fields.

## Failure vocabulary

The `PackageError` symbols below are assigned by this contract (owner
adoption, governance wave). Each names a refusal the in-process
package paths produce today; code spelling, precedence, numeric
assignment, and RW-075/RW-080 semantics are unchanged by this table.

- `PACKAGE_OVERSIZED` — LIVE. An envelope or section exceeds its
  ceiling: the type encoder (`encode_type_expr`, output over ceiling),
  the constants section over `EXEC_PACKAGE_MAX_CONSTANTS_BYTES`, and
  the approval paths that re-check section bounds.
- `PACKAGE_MALFORMED` — LIVE. A tag, shape, or count the frozen layout
  cannot carry: type depth over `MAX_TYPE_DEPTH`, an unencodable
  constant value, and an ill-formed imports manifest.
- `PACKAGE_BINDING_MISMATCH` — LIVE. A complete-closure binding does
  not match: gate fingerprints, admitted image digest, receipt profile
  digest, or entry/import consistency in `approve_package_v2`. Surfaces
  through `PackageExecutionError::Package`.
- `PACKAGE_HYDRATION_REFUSED` — LIVE. Structural hydration refused:
  duplicate identity or count bound in layouts, constants, imports, globals,
  or contracts.
- `PACKAGE_SECTION_DIGEST_MISMATCH` — LIVE on a header-bound section mismatch
  in `decode_package_envelope_v2`; ordinary in-process binding checks continue
  to report content mismatches as `PACKAGE_BINDING_MISMATCH`.

The framing codes `PACKAGE_UNKNOWN_MAGIC`, `PACKAGE_UNSUPPORTED_VERSION`,
`PACKAGE_TRUNCATED`, and `PACKAGE_TRAILING_DATA` are LIVE on the provisional
RW-080 decoder. This does not make the candidate accepted runtime authority;
the lane review remains outstanding.

## Authority failure vocabulary (owner adoption)

The `AuthorityError` symbols below are assigned by this contract (owner
adoption, governance wave). Each names a refusal the staged authority
(`sley_vm::admission_authority::admit_v2_package`) produces today; code
spelling, precedence, numeric assignment (none: the authority carries no
numerics at this revision), and RW-075/RW-080 semantics are unchanged by
this table. No receipt is minted on any of these paths.

- `AUTHORITY_UNKNOWN_ENTRY` — LIVE. The entry id matches no function in
  the closure bundle.
- `AUTHORITY_GATE_REFUSED` — LIVE. The successor-profile gate judgment
  refused the closure; no report exists.
- `AUTHORITY_REFERENCE_MISMATCH` — LIVE. Native reference re-lowering
  failed, or its bytes differ from the candidate image.
- `AUTHORITY_CLAIMS_MISMATCH` — LIVE. Package bytes, tables, or bindings
  diverge from the judged closure (gate counts/fingerprints, entry/epoch/
  root binding, carried tables, admitted image digest).
- `AUTHORITY_DIGESTS` — LIVE. `package_digests_v2` failed (bounds or
  encoding); wraps the underlying `PackageError`.
- `AUTHORITY_APPROVAL_MISMATCH` — LIVE. The minted receipt failed the
  approval cross-check; no receipt is released.
- `AUTHORITY_SLEY_EVIDENCE_UNAVAILABLE` — LIVE. The reserved Sley-produced
  admission ingress was called before C1 exists; it always refuses.

## Package observation preimage (`SLEYPOBS1`)

The package-bound observation identity is
`ObservationId = BLAKE3-256("sley2.observation.v1" || preimage)` with the
preimage laid out exactly as `observation_preimage_package` in
`crates/sley-vm/src/execute.rs` builds it (all integers big-endian):

```text
"SLEYPOBS1" || u32(1)
|| schema_epoch[32] || ssmc1_field_schema_hash[32] || ssmc1_decoder_limits_hash[32]
|| state_root[32] || entry_function[32] || cache_key[32]
|| package_digest[32] || image_digest[32] || constants_digest[32]
|| layouts_digest[32] || imports_digest[32] || dependency_digest[32]
|| profile_digest[32] || u32(host_abi_version)
|| u32(vm_major) || u32(vm_minor) || u32(vm_patch) || u32(1)
|| u64(input_count) || input_value_hash[32]...
|| u64(max_instructions) || u64(max_fuel) || u64(max_value_units) || u64(max_output_units)
|| u32(1) | u32(2) || u64(cancel_at_fuel)
|| termination (encode_termination_package)
|| u64(instruction_count) || u64(fuel_used) || u64(peak_value_units)
|| u64(0) || u64(0) || u64(0) || u64(0)
```

The `SLEYPOBS1` magic and the six package digests make a package observation
prefix-disjoint from the S20-270 `SLEYOBS1` observation under the same domain.
Frozen vectors for the v2 package digest, the five section digests and this
observation over the RHW1 bridge fixture are pinned by
`crates/sley-vm/tests/rw075_raw_callable.rs`
(`v2_package_section_digests_and_observation_are_frozen`); moving any of them
requires a new package or observation version.

## Implementation erratum E1 (V1 profile digest literal)

`EXEC_PACKAGE_V1` binds `BOOTSTRAP_PROFILE_1` by its frozen digest
`4f2691504b5c756eae1f5ef01e6e998cc4cd628d4b524b038b10d583bfefd630`
(`BOOTSTRAP_PROFILE_1.md`, `conformance/bootstrap-profile/v1/SHA256SUMS`).
Until 2026-09-08 the Rust literal `BOOTSTRAP_PROFILE_1_DIGEST` in
`crates/sley-vm/src/exec_package.rs` carried a shifted hex transcription of
that value (`...756ea1f5ef01e6e998cc4cd628d4b524b038b10d583bfefd6330`), and
the marker checker pinned only its first four bytes. Every v1 package digest,
v1 receipt and v1 package observation the implementation computed before that
date therefore bound a value the contract never defined.

Disposition (architecture-tightening finding AT-HH-01, C_PRE_FREEZE_REPAIR):

- The frozen contract meaning is unchanged; the implementation was corrected
  to it. This is a repair of the implementation, not a redefinition of the
  V1 identity.
- No v1 package digest, receipt, or observation identity computed from the
  old literal is persisted in any record, fixture, evidence file, review log
  or lane record (searched at the repair commit; the old literal's hex
  appears nowhere in the tree). Any such value that exists outside the
  repository is non-evidence and must not be cited.
- `scripts/check_exec_package_markers.py` now binds all 32 bytes of both
  profile-digest literals to their records.
- V1 remains "preserved functional for legacy evidence only" as stated above;
  new executions use v2, whose literal was always correct.

## Implementation erratum E2 (package input integer width, 2.0.1)

`execute_approved_package` and `execute_approved_package_v2` admit runtime
inputs structurally: exact register-type equality, canonical codec form,
capped value units, and the codec-plus-hash. They never run
`check_constant`, and the codec carries integer widths without comparing
data against them. Through 2.0.0 an input such as `UInt(8)` carrying `1000`,
or `SInt(64)` carrying `i128::MIN`, therefore reached execution. After the
#7 kernel fix it could no longer abort the host, but `int_shr_checked` could
still return `Ok` with a value outside the declared width, `int_div_checked`
and `int_rem_checked` could return in-width answers computed from such an
operand, and the value-unit walk recursed over caller data before the codec
had bounded its depth.

Disposition (2.0.1):

- Canonical form on the package path includes integer width, as the
  `MUTATION_VALUE_CODEC_V1` canonical rules already state ("exact widths").
  Every integer inside an input, at any depth, must carry data of its
  declared signedness that fits its declared epoch-1 width, and integer
  data may not appear under a non-integer type. Each value is compared
  with its own `value_type` only: no definition lookup, environment, or
  semantic judgment, so the host boundary above is unchanged. A violation
  is refused before execution with the existing
  `VM_EXEC_INPUT_NOT_CANONICAL` (27006) and no outcome. No new code.
- Per input the order is now: register-type equality, canonical form
  (codec, then integer width), capped value units, value hash. Canonical
  form moved ahead of unit accumulation so the codec's depth bound holds
  before anything recurses over caller data. An input that fails both
  checks now reports `VM_EXEC_INPUT_NOT_CANONICAL` where it previously
  reported `VM_EXEC_RESOURCE_LIMIT`; every valid input is unaffected.
- Defense in depth in the checked-integer kernel
  (`VM_EXTENDED_OPCODE_PROFILE_V1.md`, E2 note): the width comes from the
  result register, and an operand outside it faults as
  `VM_EXEC_INTERNAL_INVARIANT`, so no checked operation answers from an
  out-of-width value however one arrives.
- Every in-width input executes exactly as before. No envelope layout,
  digest preimage, observation preimage, frozen vector, or machine record
  changes. Regression fixtures:
  `crates/sley-vm/tests/package_input_width.rs` (public surface, both
  package versions).

## Hydration (unchanged boundary, serialized path implemented)

Same allows/forbids as v1 (byte/framing decode, digest verification,
bounds before allocation, allocation, structural hydration,
exact-equality checks; never shape/reference/cycle/map-key/typechecking/
inference/repair). The v2 serialized path implements these allowed mechanics
through `hydrate_package_envelope_v2`; its result is unadmitted until matched
to separately minted authority evidence.
