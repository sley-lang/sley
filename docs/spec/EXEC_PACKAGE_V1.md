# EXEC_PACKAGE_V1

Status: frozen (RW-075, 2026-09-06). Version 1.

- Package digest record: `9e20da24a3b3647d15d052ce759ed9b1ca7682d950421baf48978ad59a5795d4`
  (raw-byte SHA-256 of `conformance/exec-package/v1/exec-package.json`;
  `scripts/check_exec_package_v1.py` verifies the binding on every
  `make quick`).
- Contract: `sley2-exec-package-1`. Machine record:
  `conformance/exec-package/v1/exec-package.json` (authoritative for
  values); this document (authoritative for rationale and rules).
- Rust surface: `sley_vm::exec_package` (package/envelope, section codecs,
  digests, structural hydration, approved binding) plus
  `sley_vm::execute_approved_package` (validation-before-execution runner
  over the complete `ApprovedExecutionPackage` binding). Conformance
  fixtures: `crates/sley-vm/tests/rw075_exec_closure.rs` (19 tests, public
  surface only) and `crates/sley-vm/tests/rw075_hydration_workloads.rs`
  (AR-04 probes plus AR-05 workloads, public surface only).

## What this is

`EXEC_PACKAGE_V1` is the complete execution-package closure that repairs
the RW-070 `ApprovedImage` gap (AR-01/AR-03). The final loaded execution
path no longer requires Rust to semantically construct the
program-definition environment: the Sley compiler owns constants, layouts,
and semantic judgments, and the native host hydrates structurally through
`TypeEnvironment::hydrate_verified_definitions` (duplicate-identity and
count-bound checks only, never `TypeEnvironment::new`).

It is a package/envelope around the existing `SLEYBC02` image, not a new
image format. `SLEYBC02` version 1, its loader, and its byte layout are
unchanged; the envelope wraps image bytes plus the compiler-owned closure
and binds every section by digest.

It broadens nothing. `BOOTSTRAP_PROFILE_1`
(`4f2691504b5c756eae1f5ef01e6e998cc4cd628d4b524b038b10d583bfefd630`,
contract `sley2-bootstrap-profile-1`) is bound by digest, never extended.
`HOST_ABI_V1`
(`e6de00b820a094ec2abc7a6ae43263d7340c2bf1a38a6231e7426fda0f04ecc2`,
contract `sley2-host-abi-1`) is cited, never rewritten. `host-boundary.json`
(`d935d238a4d75d154df128aad630411ca3fdcc18e4db084dcd2317ab73bdb18a`)
is cited, never rewritten.

## Compiler-owned versus native information

Compiler-owned (correctness derives from Sley semantics; produced by the
Sley checker/lowerer, bound by digest, trusted via receipt): named type
definitions and record-field/variant-case layouts; typed constants and
their well-formedness; map-key orderability and hashability and
persistability traits; definition reference integrity and cycles and type
well-formedness; import-row semantic admissibility; entry designation and
dependency-closure membership.

Native (mechanical structures to execute an approved package safely):
byte/framing decode (with the `MAX_TYPE_DEPTH` nesting bound and
per-level byte ceilings, so attacker-shaped types cannot overflow the
framing before refusal); SHA-256 digest verification; bounds checking before
allocation; allocation; exact-equality binding checks; structural layout
hydration (duplicate-identity and count bounds only); register/frame
setup; primitive value execution; control-flow execution; resource
accounting; cancellation; output/result construction. Anything else
requires explicit review.

Native hydration MUST NOT perform definition-shape semantic judgment,
resolve Sley references semantically, discover definition cycles,
determine map-key/hashability rules, reconstruct record schemas from
high-level definitions, typecheck, or repair/infer missing layout data.
Where runtime memory safety requires local structural checks (duplicate
identities, count ceilings, operand-count versus field-count agreement,
member-ID presence), those are specified as structural and are distinct
from language semantic judgment: they prevent out-of-bounds memory access
on already-admitted data and never decide whether a program is well-typed.

## Bindings

The accepted execution authority binds the exact complete closure:
package digest (over the envelope of at most 67_108_864 total bytes —
image plus sections, enforced independently of the per-section
sub-budgets); image digest; complete runtime data/constants digest;
complete runtime-layout digest; exact import-row manifest digest (full
rows, never IDs alone); complete dependency/inventory digest (at most
8_388_608 bytes, 1_000_000 globals, 1_000_000 contracts); re-derived
cache key; entry; schema epoch; state root; profile (`EXTENDED_V1`);
`BOOTSTRAP_PROFILE_1` digest; VM semantic version `[1, 0, 0]`; host ABI
version 1; admitted limits (the request must match exactly); the admission
receipt for that exact package digest; and the admission authority's gate
report for the closure (entry-first function equals the package entry,
admitted import set equals the package import-identity set, operation and
bridge counts equal the digested gate claims, closure fingerprints equal
the digested gate fingerprints, and the committed admitted image digest
equals the package image digest re-derived by approval and execution —
verified without running the gate, so no report exists for a
gate-refused out-of-bootstrap closure and no report replays against
different executable bytes, including same-opcode rewiring). The host
verifies those bindings before execution. The admission evidence comes
from the separately accepted authority for that package — which verifies
builder faithfulness by reference re-lowering comparison before minting
any receipt (production: the Sley build driver per the RW-080 contract;
RW-075: modeled in tests) — and the Rust semantic checker is never rerun
as execution-time proof.

Out-of-profile images cannot execute merely because the shared runtime
supports a broader operation: a broader-profile image under a bootstrap
receipt refuses by digest/binding mismatch, and a receipt for another
package refuses with `PACKAGE_RECEIPT_MISMATCH`. The residual
trusted-staging assumption is explicit: in-process pure-data receipts are
self-mintable by any native caller, exactly like every existing native
gate, checker, oracle, and evidence builder in this repository — all of
which are secured by the SH2 campaign-declaration rule plus independent
review, not by cryptography. What the host boundary does guarantee is
exact consistency across every leg it can verify structurally (entry,
imports, counts, fingerprints, image bytes, digests, receipt), with sealed
report construction so the only obtainable genuine evidence is the gate's
own output; builder faithfulness (graphs-to-image correspondence) is
verified by the authority's reference re-lowering comparison and retired
by the RW-130/RW-140 fixed-point and self-change evidence. Execution observations
use the `SLEYPOBS1` domain plus every section digest, so two distinct
packages cannot produce interchangeable authority evidence, and package
observations can never equal legacy `SLEYOBS1` observations. Package
digests are never converted into installation authority:
candidate/judge separation remains controlling.

## Structural metadata refusals

Forged layout, missing layout, mismatched constants, wrong field layout,
wrong dependency closure, mismatched root/inventory, and malformed
structural metadata each refuse with the `PACKAGE_*` vocabulary before
anything runs. Nothing is repaired or inferred.
