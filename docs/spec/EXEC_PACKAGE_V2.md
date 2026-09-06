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
fields). Limits bind the observation post hoc, not the approval.

## Hydration (unchanged)

Same allows/forbids as v1 (byte/framing decode, digest verification,
bounds before allocation, allocation, structural hydration,
exact-equality checks; never shape/reference/cycle/map-key/typechecking/
inference/repair).
