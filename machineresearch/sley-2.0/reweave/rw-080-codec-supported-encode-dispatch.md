# RW-080 §1.1 program slice 9: supported-kind Sley encode dispatch — provisional C0 construction record

Status: PROVISIONAL (2026-09-17, operator development override
`rw075_correction.operator_override_2026_09_07`). This slice adds the
encode half of the bounded EntryPoint/Namespace dispatch proof. It is
development evidence, not accepted runtime authority. RW-080 remains
BLOCKED and R2 remains NOT_READY with all independent review and
acceptance debt unchanged.

Current extension: slice 17 adds canonical PolicyBinding kind 17 while
preserving EntryPoint, Namespace and DependencyBinding. See
`rw-080-codec-policy-binding-compose-encode.md` for the shared entity-set sum,
four-arm round trip, rejection coverage and current resource measurements.
Sections 1 through 4 retain the original slice-9 record; section 5 retains the
slice-13 DependencyBinding extension.

## 1. Scope and value shape

`encode_supported_program(declared_kind: UInt64, value:
SupportedValue, unit: Unit) -> Result<Bytes, Bytes>` accepts the same
closed semantic sum returned by the slice-8 decoder:

- inner `Ok`: EntryPoint
  `(entity_id: Bytes, function: Bytes, exposure: UInt64)`;
- inner `Err`: Namespace
  `(entity_id: Bytes, parent: Bytes, members: Bytes)`.

Namespace member count is intentionally absent from the shared value:
it is wire framing derived by the proven Namespace encoder, not an
independent semantic field. Slice 8 was tightened to the same shape,
and a cross-image test now feeds each decoder result directly to the
encoder and recovers the exact stored bytes.

Declared kind 16 requires the EntryPoint arm; kind 3 requires the
Namespace arm. A supported-kind/arm mismatch returns
`SSMC_RESERVED_FIELD_PRESENT`. Other known kinds 1 through 18 return
the same explicit scope code, while 0 and 19 or greater return
`SSMC_ENTITY_KIND_UNKNOWN`.

This is still an explicit-kind bounded proof. It does not claim the
final schema-bearing `codec_main`, other entity bodies, optional outer
metadata, or a completed program/schema codec.

## 2. Construction

- `crates/sley-vm/tests/rw080_codec_program/supported_encode_dispatch.rs`
  owns the new dispatch graph and three tests.
- The graph selects and validates the semantic arm, unpacks its typed
  tuple, then calls the previously proven complete EntryPoint or
  Namespace program encoder. Those paths perform body encoding, outer
  record encoding, envelope construction, and digest emission inside
  Sley.
- The image shares one uvar encoder, outer encoder, and envelope
  builder. The two body/program encoders remain distinct. Constants are
  coalesced with the same helper used by slice 8.
- The child modules share the result-shape, block-append, and constant
  coalescing helpers, plus the decode harness entry used by the
  cross-image round-trip test. The retained 29k-line parent gains only
  the module declaration.
- Imports stay limited to the frozen `HOST_ABI_V2` B2V1, PSH1, V2B1,
  and RHW1 rows. No native semantic codec or protected-limit change was
  introduced.

## 3. Evidence

The tests prove:

- independently supplied EntryPoint structured values emit the exact
  native canonical 153-byte stored object;
- independently supplied empty Namespace structured values emit the
  exact native canonical 123-byte stored object;
- both supported-kind/arm mismatches fail closed;
- known-unsupported kind 1 and unknown kinds 0/19 preserve the exact
  declared-kind refusal classes; and
- decode then encode through separately admitted supported images is
  byte-exact for both sum arms.

Under the unchanged codec limits:

| Kind | Fuel | Instructions | Peak value units |
|---|---:|---:|---:|
| EntryPoint 16 | 21,932 | 2,739 | 902,329 |
| Namespace 3, no parent or members | 16,275 | 2,152 | 586,643 |

The EntryPoint path remains below the protected one-million-unit cap
with less headroom because the image contains both complete program
encoders. Larger Namespace shapes retain the slice-7 F5 envelope; this
slice neither widens the limit nor claims unsupported sizes.

## 4. Validation and remaining work

Targeted validation:

- `cargo test -p sley-vm --test rw080_codec_program_outer` (38 tests)
- `cargo clippy -p sley-vm --test rw080_codec_program_outer -- -D warnings`
- `cargo test -p sley-vm --test rw080_codec_uvar`
- `cargo test -p sley-vm --test rw080_codec_envelope`
- `cargo test -p sley-vm --test rw080_codec_scaffold`
- `cargo test -p sley-scb1 --locked --lib`
- `cargo test -p sley-mutate --locked --lib object`
- `cargo fmt --all -- --check`
- `python3 scripts/build_anti_goal_conformance.py --check`
- `git diff --check`

Local strict review checked sum-shape identity, arm/kind mismatch
handling, absence of host-side encoding, constant-reference rewrites,
and frozen-limit preservation. Independent review was unavailable, so
the result remains provisional and no gate, ledger, or authority state
changes.

The next fixed-width format now has paired DependencyBinding body
encode and strict decode records
(`rw-080-codec-dependency-binding-encode.md` and
`rw-080-codec-dependency-binding-decode.md`). Program decode and encode
composition now extend both supported entries in
`rw-080-codec-dependency-binding-compose-decode.md` and
`rw-080-codec-dependency-binding-compose-encode.md`.
Label/NFC/fingerprint work remains separate because its required text and
verification machinery has not landed.

## 5. Slice-13 additive state

The current entry supports kinds 3, 16, and 18. Its semantic input is
`Result<Result<EntryPoint, Namespace>, DependencyBinding>`, where the new arm
carries `(entity_id, dependency_root, external_package, local_namespace)` as
four exact 32-byte values. Decode-to-encode round trips are byte-exact for all
three arms. Canonical kind 18 measures 3,757 fuel, 1,031 instructions, and
416,321 peak value units for the 219-byte fixture under the unchanged limits.

## 6. Slice-17 additive state

The current entry supports kinds 3, 16, 17 and 18 and consumes the same compact
semantic sum emitted by the decoder. Namespace and PolicyBinding share one
runtime-mode entity-set body graph. The kind-17 empty-requirements fixture
emits the exact native 153-byte stored object at 25,705 fuel, 3,314
instructions and 956,464 peak value units. The known F5 combined-image size
envelope is retained and explicitly tested for a one-requirement kind-17
value. Full construction and refusal evidence is in
`rw-080-codec-policy-binding-compose-encode.md`.

## 7. Slice-23 additive state

The current entry supports kinds 2, 3, 16, 17 and 18 and consumes the exact
five-arm sum emitted by the supported decoder. Package kind 2 carries
`(2, entity_id, canonical_body, empty_reserved)` and emits the native 190-byte
stored object at 20,283 fuel, 2,139 instructions and 936,965 peak value units.
Decode-to-encode round trips are byte-exact for all five arms. The specialized
witness composer, rejection matrix, discarded general composition and current
resource measurements are recorded in
`rw-080-codec-package-supported-encode.md`.

## 8. Slice-25 additive state

The current entry consumes the same uniform six-field tagged tuple as the
decoder and supports kinds 1, 2, 3, 16, 17 and 18. Workspace and Package share
one declared-kind-aware fixed-body checker and one witness composer while
retaining byte-exact native output. Canonical Workspace emits at 17,683 fuel,
2,044 instructions and 726,313 peak value units. Decode-to-encode round trips
are byte-exact for all six kinds. Full construction and measurements are in
`rw-080-codec-workspace-supported-dispatch.md`.
