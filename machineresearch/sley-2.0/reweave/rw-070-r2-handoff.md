# R2 architecture handoff — REWEAVE bootstrap-ready foundation

Purpose: independent premium-model review before the canonical Sley
self-hosted toolchain graph is created (RW-080). No RW-080 implementation
in this run. No push, publication, provider spend, SH3, full MCR1, general
FFI, or unrelated feature work.

## 1. Exact current commit

- HEAD: `b4c3390088deed018f818eb1327f130002c27ab6` (`main`), tree holding
  the uncommitted RW-070 package only (no unrelated deltas; no reset of
  the `origin/main` sanitized-history divergence, which has no merge-base
  and is preserved).
- RW-070 package files: `docs/spec/HOST_ABI_V1.md`;
  `conformance/host-abi/v1/{host-abi.json,SHA256SUMS}`;
  `crates/sley-vm/src/host_abi.rs`; `crates/sley-vm/tests/rw070_host_abi_freeze.rs`;
  `crates/sley-vm/src/{lib,bootstrap,bootstrap_closure,execute,extended}.rs`
  (owned-scope repairs); `scripts/check_host_abi_{v1,markers}.py`;
  `scripts/{build_independent_conformance_report,check_bootstrap_gate_markers,check_supply_chain_audit}.py`
  (registry/marker/pin updates); `Makefile`; `Cargo.{toml,lock}`
  (`sha2` alloc-only image-identity selection);
  `machineresearch/sley-2.0/{machine-summary.json,reweave/rw-070.md,
  reweave/rw-070-host-inventory.json,reviews/reweave-rw070-*.log}`;
  `evidence/security/T52/pre-release-inventory.json`,
  `evidence/security/T54/secret-scan.json` (owned-builder regen).

## 2. Frozen identities

- `BOOTSTRAP_PROFILE_1`: `4f2691504b5c756eae1f5ef01e6e998cc4cd628d4b524b038b10d583bfefd630`
  (contract `sley2-bootstrap-profile-1`, 42 opcodes, 20 closure vectors).
- `host-boundary.json`: `d935d238a4d75d154df128aad630411ca3fdcc18e4db084dcd2317ab73bdb18a`
  (contract `sley2.host-boundary.v1`, byte-stable, cited never rewritten).
- `HOST_ABI_V1`: `e6de00b820a094ec2abc7a6ae43263d7340c2bf1a38a6231e7426fda0f04ecc2`
  (contract `sley2-host-abi-1`, version 1).
- Import identities (all `abi_version` 1, `Index` failure, empty effects):
  B2V1 `534c59312f4252494447452f4232563100…0000`;
  V2B1 `534c59312f4252494447452f5632423100…0000`;
  PSH1 `534c59312f4252494447452f5053483100…0000`.
- Image: `SLEYBC02` version 1, SHA-256 identity, 8-code `IMAGE_*`
  vocabulary, `ApprovedImage` binding.
- Bindings: VM `[1,0,0]`, lowering profile 2, lowerer `[2,0,0]`, epoch
  `08`*32, 12-field cache preimage, toolchain 1.93.0 minimal.

## 3. Complete admitted native dependency list

REQUIRED_PORTABLE_NATIVE_PRIMITIVE (pure, side-effect-free, portable):
B2V1 (Bytes→Vector⟨UInt(8)⟩), V2B1 (reverse, exact u8 only), PSH1-family
(one row per closed inventory; response exactly Vector of request).
REQUIRED_HOST_RUNTIME_MECHANIC: bounded EXTENDED_V1 execution (fuel,
256-frame, 1M-cell/value/input caps, deterministic cancellation);
allocation/fuel accounting; whole-value primitive ops incl. `value_hash`;
SLEYBC02 encoding; cache-key/observation derivation; structural loader
(`load_image`) and inventory-bound runner (`execute_loaded_image`).
Full table: `rw-070-host-inventory.json` (zero UNKNOWN rows).

## 4. SH2 native remainder

Seed-reference crates stay outside the bootstrap closure: store/txn/
state-root/repo/query/mutate/policy (persistence, candidates, roots),
protocol/adapter/cli/json-bridge (surfaces), conformance/oracles/drivers
(permanently outside the clean self-build closure). Sley-owned
responsibilities with seed references: SCB codec (RW-090), semantic
checker (RW-100), lowerer/image-builder (RW-110), build driver (RW-120),
Witness checker (RW-170), discharge semantics (RW-180). No native compiler
service is reachable from the toolchain closure (proved §5).

## 5. Anti-shortcut evidence

37 integration fixtures (public surface only) + 3 `host_abi` unit pins + 2
gate-closure units + E8 adversarial/declaration/fuzz lanes: 8
compiler-service spellings denied (`VM_LOWER_OPCODE_UNSUPPORTED`), helper
injection denied, unreferenced/duplicated rows denied, tamper classes
refuse through the loader (`IMAGE_*`), digest/binding mismatches refuse
before execution, hygiene scan (no unsafe/FFI/fs/env/process/net in
closure crates). No arbitrary symbol lookup, dlopen, hidden Cargo/rustc
paths, registry-broadening switches, or ABI-altering test features.

## 6. RW-060 lifecycle identity (preserved, unaffected)

Genesis root `c2e16e8a…45aa83d` → committed root `dfffde27…305bc6e`
(tx `8cf379c4…bde0c`, receipt `a33ca740…9dae`); reject P7
`VM_LOWER_SIGNATURE_MISMATCH`/26002; observations `b2491550…3d547`
(ok) / `55f9651f…2867f0` (err); pack `70f16b57…c0e13e`; 15 driver tests
green in the closing tree.

## 7. Staged-checker result

`check_bootstrap_capability.py`: audit PASS, R2 readiness READY, zero
open bootstrap blockers, S/C0–C3 correctly later-stage-unbound (11 corpus
cases incl. the 2 R2-gate regressions).

## 8. Package states

RW-030 COMPLETE; RW-040 COMPLETE; RW-050 COMPLETE (BOOTSTRAP_PROFILE_1
frozen); RW-060 COMPLETE (reviewed lifecycle); RW-070 COMPLETE (this
package, Nabu PASS + confirmation, Ariadne PASS, all failed rounds
preserved). No RW-080/C1/self-hosted compiler work begun.

## 9. BOOTSTRAP_READY verdict: TRUE

Adopted R2 conditions — P frozen + RW-060 lifecycle + RW-070 freeze —
all pass; later C0/C1/C2/C3, RW-080+, WITNESS, GOLD/release, and packaging
did not block this R2-only decision. This is the bootstrap-ready
foundation, not a full-language or self-hosting claim.

## 10. Carried findings and evidence gaps

- Release lane (owned elsewhere, unchanged): quick halts at the
  release-candidate packaging check (59 tests, errors=15 for the absent
  candidate — byte-identical at clean `b4c3390`); decision_state BLOCKED;
  T52/T54 PASS with global DEFERRED on the root license.
- Repaired in passing (preserved as history): clean-tree count 1112 vs
  closed-record 1102; T52 staleness at baseline (RW-060 dev-deps);
  RW-050 gate-marker evolution (`bootstrap_row_ok` + `referenced`).
- Open by design: multi-push-type closures need §18 admission; C1/C2/C3
  fixed-point and self-change evidence (RW-130/RW-140) retires the stated
  residual trust root; multi-platform qualification unclaimed.

## 11. Exact first R3 package and proposed ownership boundary

RW-080 (Canonical Sley toolchain graph and seed construction; depends on
RW-060 + RW-070; minimum gate: graph root, construction manifest, no
source DSL). Proposed boundary: the R3 owner owns the Sley toolchain
program graph S, its dependency closure, and the construction manifest;
the RW-070 native host/ABI (this freeze) is read-only shared ground —
usable, citable, but not modifiable without a new reviewed amendment; the
Rust seed (lowering/execution/reference crates) stays available as
algorithm source and staged-image builder until the R3/R4 seed-absence
proof removes it per the frozen seed-absence condition.
