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
returns raw canonical section bytes and performs no semantic hydration or
admission judgment. Its candidate vector and independent Python reproduction
are under `conformance/exec-package-envelope/v2/`. The package identity and
all existing section encodings are unchanged. The envelope candidate remains
provisional until the RW-080 contract/surface review accepts it.

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
  duplicate identity or count bound in `hydrate_layouts`, and imports
  manifest refusal.
- `PACKAGE_SECTION_DIGEST_MISMATCH` — LIVE on a header-bound section mismatch
  in `decode_package_envelope_v2`; ordinary in-process binding checks continue
  to report content mismatches as `PACKAGE_BINDING_MISMATCH`.

The framing codes `PACKAGE_UNKNOWN_MAGIC`, `PACKAGE_UNSUPPORTED_VERSION`,
`PACKAGE_TRUNCATED`, and `PACKAGE_TRAILING_DATA` are LIVE on the provisional
RW-080 decoder. This does not make the candidate accepted runtime authority;
the lane review and semantic section hydration remain outstanding.

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

## Hydration (unchanged)

Same allows/forbids as v1 (byte/framing decode, digest verification,
bounds before allocation, allocation, structural hydration,
exact-equality checks; never shape/reference/cycle/map-key/typechecking/
inference/repair).
