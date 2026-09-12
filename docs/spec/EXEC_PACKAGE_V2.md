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

No canonical byte serialization of the envelope (header layout, section
order, length prefixes) is frozen by this contract, and no encoder or
decoder for one exists. Packages reach `execute_approved_package_v2` as
in-process structures. The framing codes `PACKAGE_UNKNOWN_MAGIC`,
`PACKAGE_UNSUPPORTED_VERSION`, `PACKAGE_TRUNCATED` and
`PACKAGE_TRAILING_DATA` are reserved for the strict decoder that the
RW-080 builder/loader handoff will freeze together with its emitter and
vectors; until then they are unreachable by design. Freezing the layout
will not change the package digest preimage.

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
- `PACKAGE_SECTION_DIGEST_MISMATCH` — RESERVED/DEAD. No construction
  site exists: the binding checks report content mismatches as
  `PACKAGE_BINDING_MISMATCH`. Reserved for a header-bound section
  comparison, mirroring the decoder-reserved framing codes above.

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
