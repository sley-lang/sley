# RW-090 arbitrary four-leg codec composition

Date: 2026-09-18

Status: implemented candidate composition; the retained bounded `codec_main`, its canonical codec component, and the preserved C0/C1 candidate are unchanged

## Result

`arbitrary_codec_main_image` composes the same four legs as
`rw-090-codec-main.md` — program decode, program encode, schema decode, and
schema encode behind one selector entry — with selector 0 routed to the
arbitrary-schema all-kind decoder (`rw-090-arbitrary-all-kind-dispatch.md`)
instead of the bounded digest-pinned aggregate. The composer, the encode leg,
the schema legs, the selector contract, the uniform typed result, and the
four frozen bootstrap bridges are exactly those of the bounded composition;
the bounded `codec_main_image` remains and is the source of the canonical
codec component and the preserved candidate.

A dedicated test proves the arbitrary decoder shares no identity namespace
with the other three children, so the merge is a plain inventory union.

## Executable surface

The composed image contains 133 functions, 6,240 parameters, 1,908 blocks,
4,162 operations, 176 deduplicated constants, and 4 adapter imports. Its
encoded image is 500,596 bytes. The approved package digest is
`0aef4b47d8a48724a5521da5552a513812754b3c482e504b0b2988cb4eb000de`
under the codec-profile execution limits.

## Measured evidence

Through the one admitted entry:

- selector 0 decodes all 18 representative native objects of the bounded
  profile and all 32 rich objects that the bounded composition refuses with
  `SSMC_RESERVED_FIELD_PRESENT` (the test executes each rich object against
  both compositions and pins both verdicts);
- selector 1 re-emits every representative object byte for byte;
- selectors 2 and 3 round-trip the frozen schema preimage;
- an unknown selector returns `VERSION`; a trailing byte after a body union
  forwards `SCB_TRAILING_BYTES` from the schema decoder.

Peak decode cost across the rich corpus is 656,175 fuel, 72,993
instructions, and 35,732,855 value units (the 862-byte replay TestCase),
inside the profile's 10,000,000 fuel, 100,000 instructions, and 100,000,000
value units.

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

The program encode leg is still the bounded witness emitter; arbitrary
encoding is not claimed.
