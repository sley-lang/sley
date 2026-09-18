# RW-090 arbitrary four-leg codec composition

Date: 2026-09-18

Status: implemented candidate composition; the retained bounded `codec_main`, its canonical codec component, and the preserved C0/C1 candidate are unchanged

## Result

`arbitrary_codec_main_image` composes the same four legs as
`rw-090-codec-main.md` — program decode, program encode, schema decode, and
schema encode behind one selector entry — with both program legs arbitrary:

- selector 0 is the arbitrary-schema all-kind decoder
  (`rw-090-arbitrary-all-kind-dispatch.md`); and
- selector 1 judges the supplied body with the same schema decoders and then
  re-frames it through the generic outer-record and envelope composer
  (`build_program_payload_encode`: canonical two-field outer record, then
  the `SLEYSCB1` envelope with its RHW1 object digest), so every canonical
  body of every kind encodes, not only the seventeen pinned witnesses. Kind
  18 keeps its retained emitter.

The two program legs live in one image (`arbitrary_program_legs_image`)
because they share one schema closure; the 256-namespace budget cannot hold
two copies. The composer, the schema legs, the selector contract, the
uniform typed result, and the four frozen bootstrap bridges are exactly
those of the bounded composition; the bounded `codec_main_image` remains and
is the source of the canonical codec component and the preserved candidate.

A dedicated test proves the program legs share no identity namespace with
the schema legs, so the merge is a plain inventory union.

## Executable surface

The composed image contains 119 functions, 6,776 parameters, 1,972 blocks,
4,185 operations, 145 deduplicated constants, and 4 adapter imports. Its
encoded image is 518,352 bytes. The approved package digest is
`1d57aba7f4ceee744e7c19183699f236aabdca1fa9e68fe73408a0fcd7e20b5c`
under the codec-profile execution limits. (A first derivation of this
composition, with selector 0 arbitrary and selector 1 still the bounded
witness emitter, admitted at 133 functions and 500,596 bytes with digest
`0aef4b47…`; it is superseded by this record.)

## Measured evidence

Through the one admitted entry:

- selector 0 decodes all 18 representative native objects of the bounded
  profile and all 32 rich objects that the bounded composition refuses with
  `SSMC_RESERVED_FIELD_PRESENT` (the test executes each rich object against
  both compositions and pins both verdicts);
- selector 1 re-emits every representative object and every rich object
  byte for byte from `(entity, body)` alone, matching the native
  `build_entity_object` bytes;
- selectors 2 and 3 round-trip the frozen schema preimage;
- an unknown selector returns `VERSION`; a trailing byte after a body union
  forwards `SCB_TRAILING_BYTES` from the schema decoder on both program
  legs, and a 31-byte entity on the encode leg is `SCB_LENGTH_OVERFLOW`.

Peak cost across the rich corpus, over both program legs, is 656,175 fuel,
74,950 instructions, and 45,276,332 value units (the 862-byte replay
TestCase), inside the profile's 10,000,000 fuel, 100,000 instructions, and
100,000,000 value units.

```text
cargo test -p sley-vm --test rw120_toolchain_integration arbitrary -- --nocapture
cargo test -p sley-vm --test rw120_toolchain_integration
cargo test -p sley-vm --test rw080_codec_program_outer
cargo clippy -p sley-vm --test rw120_toolchain_integration -- -D warnings
cargo fmt --check -p sley-vm
```

## What this does not do

Deriving the canonical codec component from this composition, binding it
into canonical `S`, and producing a new C0-built C1 candidate would replace
the preserved seed artifacts of `rw-120-c0-c1-candidate.md` (a release-built
seed executable held outside the repository, its 179-second qualification
run, `bootstrap-manifest.json`, and the RW-120 component manifests). That
re-mint is a deliberate, operator-visible step and is left for the operator
to schedule. Until then the bounded codec remains the component authority,
and RW-090's remaining debt is exactly that derivation plus independent
review.

What the encode leg does not do: it re-frames a canonical body it is
handed; it does not construct bodies from typed fields. Field-level
construction remains native (`sley-mutate`).
