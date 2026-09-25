# RW-090 arbitrary all-kind body validation

Date: 2026-09-18

Status: SUPERSEDED as a status record (2026-09-18): `codec_main` was routed to this image, the canonical codec component was re-derived over it and canonical `S`/C0/C1 re-minted the same day (`rw-090-arbitrary-canonical-codec-objects.md`, `rw-120-c0-c1-candidate.md`); the 178873d7 Council round then replaced the kind-18 route with the strict decoder and made depth charging native (`rw-090-codec-component-manifest.json` `review_repairs`), so the measurements below describe the pre-review image. RW-090 remains open on independent review only.

## Result

The all-kind program decoder now has an arbitrary-schema variant. Where the
bounded aggregate (`rw-080-codec-all-kind-digest-dispatch.md`) judged a
decoded body for kinds 1 through 17 by comparing its digest to one pinned
representative, the new image routes the declared kind to that kind's
arbitrary schema decoder and forwards the decoder's own refusal code. The
envelope validation, outer record projection, declared-kind range check,
DependencyBinding route (kind 18), and six-field result contract are
unchanged; the bounded image and its retained evidence are untouched.

Every schema decoder is built once over one shared primitive closure (uvar,
record, union, list, fixed identity, identity collections, exact and bounded
varints, one- to six-field record projections, the recursive `TypeExpr`
chain, type-parameter lists, and the empty-payload check) under one
sequential namespace allocator that skips the host image's reserved
identities. Kinds 1 (Workspace), 2 (Package), 3 (Namespace, with its optional
parent), 16 (EntryPoint), and 17 (PolicyBinding) gained arbitrary
simple-record schemas in this slice; the remaining kinds compose the recipes
recorded in today's per-kind slices.

## Executable surface

The image contains 108 reachable functions, 5,790 parameters, 1,701 blocks,
3,034 operations, and 137 deduplicated constants. Its encoded image is
397,266 bytes. The approved package digest is
`e05708870d3d2844cde2bf173c3812067a8b6a8cf22ab77b33e05209cb1e20b4`
under the declared codec-profile execution limits (100,000 instructions,
10,000,000 fuel, 100,000,000 value units, 10,000,000 output units).

The schema closure takes identity namespace 13 and block namespaces from
`89..=91`, `96..=129`, `154..=225`, and `238..=255`, leaving every namespace
the four-leg `codec_main` composition already occupies untouched, so this
exact image merges into `codec_main` (`rw-090-arbitrary-codec-main.md`). The
digest above supersedes the first derivation of this slice, which had
reserved only the decode image's own namespaces; the graphs are identical
and only construction identities moved.

## Measured evidence

All 18 representative native objects of the bounded aggregate decode through
the arbitrary image with byte-identical results, and 32 further rich objects
decode that the bounded aggregate refuses: nonempty Workspace, Package,
Namespace, and PolicyBinding sets, a Protocol entry point, both TypeDef
forms, a full Function, a maximal Parameter, all five Block terminators, all
seven Operation immediates, nested constants in Constant and
CapabilityRequirement, a Contract with and without limits, both TestCase
environments, and a full AdapterImport. Peak cost is the 862-byte replay
TestCase at 656,148 fuel, 72,981 instructions, and 35,625,029 value units.
Representative-object decodes cost between 21,530 and 63,378 fuel.

Refusals forward unchanged: unordered Workspace packages (`SCB_MAP_ORDER`),
an unknown TypeDef form and a declared-kind mismatch (`SCB_UNION_INVALID`),
an invalid nested constant boolean (`SCB_BOOL_INVALID`), a missing Workspace
field (`SCB_FIELD_MISSING`); declared kinds 0 and 19 return
`SSMC_ENTITY_KIND_UNKNOWN` before any body work.

```text
cargo test -p sley-vm --test rw120_toolchain_integration arbitrary_dispatch -- --nocapture
cargo test -p sley-vm --test rw120_toolchain_integration
cargo test -p sley-vm --test rw080_codec_program_outer
cargo clippy -p sley-vm --test rw120_toolchain_integration -- -D warnings
```

## Remaining for RW-090 (as of 2026-09-18, after the re-mint)

- Routing `codec_main` selector 0 to this image and re-deriving the canonical
  codec component: done (`rw-090-arbitrary-canonical-codec-objects.md`).
- The program encode leg (selector 1) validates and re-frames the canonical
  body bytes it is handed for every kind (`rw-090-arbitrary-codec-main.md`);
  it does not construct bodies from typed fields, which remains native.
- Resource bound: instruction cost near 90 per body byte and the value-unit
  envelope bound a single decode under `codec_profile_limits` (100,000
  instructions, 100,000,000 value units); an identity-heavy 1,750-byte
  Workspace terminates with the VM's value-unit limit at 33,486
  instructions, instruction-heavy bodies with the instruction limit near
  1.1 KiB. Pinned by
  `arbitrary_dispatch_over_budget_body_terminates_with_a_resource_limit`
  (deleted by mistake at 0b7a1482, restored and re-measured at the fourth
  generation) and disclosed in `rw-090-codec-component-manifest.json`
  `resource_bound`.
- Refusal parity is mechanized: every per-kind rejection case for kinds
  4, 6–15 and the identity kinds 1, 2, 3, 5, 16, 17, 18 asserts the Sley
  code equals `sley_mutate::import_entity_object` on the same stored bytes
  (`assert_native_body_parity`,
  `arbitrary_dispatch_refuses_identity_kind_boundaries_with_native_codes`);
  nesting depth is charged as the native codec charges it at every site,
  every ConstData container arm and every listed sibling
  (`arbitrary_dispatch_matches_native_nesting_boundaries_at_every_site`,
  `arbitrary_dispatch_charges_listed_siblings_at_one_depth`; the db53894e
  round found and the fifth re-mint repaired a per-sibling depth creep in
  the recursive driver);
  the one case without a native counterpart is the declared-kind mismatch,
  a dispatch precondition the native codec cannot express.
- Independent review: 178873d7 round REVISE in all three lanes; repairs
  landed, re-review pending (machine summary `rw090_arbitrary_codec`).
