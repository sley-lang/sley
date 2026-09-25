# RW-080 §1.1 program slice 38: bounded all-kind aggregate codec — provisional C0 construction record

Status: PROVISIONAL (2026-09-18, operator development override
`rw075_correction.operator_override_2026_09_07`). This slice composes one
native canonical profile for every SSMC1 entity kind into paired aggregate
decode and encode images. It is development evidence, not accepted runtime
authority. RW-080 remains BLOCKED and R2 remains NOT_READY; independent review
and acceptance debt are unchanged.

## 1. Construction

The earlier aggregate dispatcher composed six specialized formats. Adding the
remaining specialized graphs directly could not satisfy the frozen
1,000,000-value-unit limit: the existing DependencyBinding decode already
approaches that ceiling. This slice keeps all richer standalone codecs intact
and adds a compact bounded aggregate profile:

- kinds 1–17 validate a declared kind and a raw BLAKE3 digest pinned to that
  kind's representative native body before returning the flat supported value;
- kind 18 follows the existing compact specialized DependencyBinding decoder,
  avoiding a second live copy of its 105-byte body;
- kinds 1–17 encode through fixed-length witness emitters selected by declared
  kind and shared exact-identity/concatenation helpers;
- kind 18 encodes through the existing specialized DependencyBinding program
  emitter;
- the decode result keeps the established six-field supported tuple. For
  kinds 1–17 it contains `(kind, entity, body, empty, empty, 0)`. For kind 18
  it contains `(18, entity, root, external_package, local_namespace, 0)`.

Every admitted fixture is built as a complete contract-200 object under epoch
`[9; 32]` and imported through the native `sley-mutate` codec before execution.
Both legs reproduce or consume the same stored bytes. A body outside its
declared pinned profile refuses with `SSMC_RESERVED_FIELD_PRESENT`; an unknown
encode kind refuses with `SSMC_ENTITY_KIND_UNKNOWN`. The host bridge remains
limited to B2V1, PSH1, V2B1 and raw-hash RHW1. No semantic host codec, runtime
limit, charging rule or authority state changed.

The bounded profiles match the standalone codec probes: empty collection and
Unit forms where applicable, fixed enum/scalar choices, and representative
identity fields. The profile is deliberately narrower than the union of the
standalone codecs. The aggregate construction proves complete kind routing
inside the protected execution limits; it does not claim full SSMC1 body
generality.

## 2. Measured evidence

The paired aggregate images admit under Bootstrap Profile V2 and execute under
100,000 instructions, 1,000,000 fuel, 1,000,000 value units and 100,000 output
units.

| Kind | Stored bytes | Decode fuel/instructions/peak | Encode fuel/instructions/peak |
|---:|---:|---:|---:|
| 1 | 162 | 24,202 / 2,823 / 579,719 | 16,660 / 1,649 / 663,717 |
| 2 | 190 | 29,551 / 3,305 / 796,148 | 19,575 / 1,931 / 878,026 |
| 3 | 123 | 16,798 / 2,164 / 364,723 | 12,610 / 1,263 / 422,786 |
| 4 | 130 | 18,105 / 2,281 / 395,850 | 13,341 / 1,335 / 461,136 |
| 5 | 172 | 26,114 / 3,001 / 651,129 | 17,712 / 1,757 / 736,387 |
| 6 | 160 | 23,863 / 2,803 / 566,478 | 16,467 / 1,639 / 649,856 |
| 7 | 171 | 26,008 / 3,000 / 644,063 | 17,614 / 1,751 / 728,982 |
| 8 | 166 | 25,009 / 2,909 / 607,730 | 17,097 / 1,703 / 692,392 |
| 9 | 127 | 17,576 / 2,244 / 382,309 | 13,044 / 1,315 / 444,572 |
| 10 | 157 | 23,253 / 2,752 / 546,660 | 16,167 / 1,617 / 629,307 |
| 11 | 138 | 19,698 / 2,439 / 435,857 | 14,194 / 1,429 / 507,751 |
| 12 | 156 | 23,069 / 2,739 / 540,272 | 16,069 / 1,611 / 622,562 |
| 13 | 190 | 29,584 / 3,327 / 796,423 | 19,608 / 1,953 / 878,301 |
| 14 | 195 | 30,563 / 3,418 / 840,622 | 20,131 / 2,005 / 920,221 |
| 15 | 168 | 25,436 / 2,961 / 622,271 | 17,326 / 1,737 / 707,081 |
| 16 | 153 | 22,511 / 2,696 / 521,454 | 15,769 / 1,589 / 602,541 |
| 17 | 153 | 22,514 / 2,698 / 521,479 | 15,772 / 1,591 / 602,566 |
| 18 | 219 | 28,271 / 3,296 / 953,390 | 3,746 / 1,027 / 345,588 |

DependencyBinding decode is the measured aggregate peak at 953,390 value
units. TestCase encode is the encode peak at 920,221. Both stay below the
unchanged protected ceiling.

## 3. Validation and review state

The slice is covered by:

- `cargo test -p sley-vm --test rw080_codec_program_outer`
- `cargo clippy -p sley-vm --tests -- -D warnings`
- the existing RW-080 uvar, envelope, scaffold, checker and lowering suites;
- `cargo test -p sley-scb1 --locked --lib`;
- `cargo test -p sley-mutate --locked --lib`;
- `cargo fmt --all -- --check`;
- `python3 scripts/build_anti_goal_conformance.py --check`;
- `python3 -m json.tool evidence/reweave/sh2-work-items.json`;
- `git diff --check`.

Local review verifies declared-kind/body binding, native import, exact stored
bytes on encode, typed refusal paths, bridge confinement and measured resource
ceilings. Independent review is unavailable, so no gate, ledger, acceptance
verdict or runtime-authority state is promoted.
