# RW-090 four-leg codec composition — provisional construction record

Status: PROVISIONAL (2026-09-18, operator development override
`rw075_correction.operator_override_2026_09_07`). This slice replaces the
four-leg trap scaffold with an executable composition for the declared bounded
bootstrap profile. It is development evidence, not accepted runtime authority.
RW-080 and RW-090 remain BLOCKED and R2 remains NOT_READY.

## Executable interface

`crates/sley-vm/tests/rw080_codec_program/codec_main.rs` constructs one
admitted Sley entry with selector values:

| selector | operation | child graph |
|---:|---|---|
| 0 | decode program | bounded all-kind program decoder |
| 1 | encode program | bounded all-kind program encoder |
| 2 | decode schema | exact frozen-registry decoder |
| 3 | encode schema | exact frozen-registry encoder |

The entry accepts a selector, declared entity kind, five byte fields, and
Unit. Those slots hold the stored object for program decode; entity/body and
profile fields for program encode; preimage for schema decode; or epoch and
record for schema encode. It returns one uniform typed
`Result<(operation, kind, bytes0, bytes1, bytes2, bytes3, count), Bytes>`.
The normalized tuple carries all six fields from program decode, the emitted
bytes from either encode leg, or the epoch and canonical record from schema
decode. Unknown selectors return typed `VERSION` rather than trapping.

Every selector comparison, direct call, success normalization, and error
forward happens in Sley. The combined image imports only the four frozen
bootstrap bridges already used by its children. No host-side codec dispatch or
semantic service is introduced.

The composite image uses the frozen profile's reference execution budget:
100,000 instructions, 10,000,000 fuel, 100,000,000 value units, and
10,000,000 output units. The earlier 1,000,000-value-unit construction budget
remains in place for the smaller standalone probes; it was a slice budget, not
the profile ceiling. The namespace reassignment in the all-kind encoder only
prevents entity-ID collision when the four previously separate images share a
single closure. It changes no graph behavior, fixture, or emitted byte.

## Declared profile and tests

The program legs cover one native canonical profile for each of the 18 closed
SSMC1 entity kinds. Program decode validates the complete object envelope,
declared kind, outer record, and the representative body. Program encode emits
the same canonical stored bytes. The richer standalone per-kind tests remain
the semantic evidence for their supported shapes. Inputs outside the pinned
profile fail closed; this slice does not claim arbitrary-program generality.

Two composition tests prove:

- selectors 0 and 1 round-trip all 18 representative native entity objects
  through the same admitted `codec_main` image;
- selectors 2 and 3 round-trip the exact frozen schema preimage;
- native schema bytes and derived epoch identity are preserved;
- a malformed schema forwards `SCHEMA_RECORD_INVALID` unchanged;
- an unknown selector returns `VERSION` as a typed error.

Validation commands:

- `cargo test -p sley-vm --test rw080_codec_program_outer codec_main -- --nocapture`
- `cargo test -p sley-vm --test rw080_codec_program_outer` (99 tests)
- `cargo clippy -p sley-vm --test rw080_codec_program_outer -- -D warnings`
- `cargo fmt --all -- --check`
- derived inventory, dossier, anti-goal, frontier, and component-manifest checks

The graph now also has a derived-identity canonical object construction record
in `rw-090-canonical-codec-objects.md`. Remaining work includes arbitrary body
shapes, optional label/NFC metadata, binding the constructed object bundle into
canonical `S`, independent byte/error parity, and formal acceptance. No gate,
ledger, release, or runtime-authority state changes in this slice.
