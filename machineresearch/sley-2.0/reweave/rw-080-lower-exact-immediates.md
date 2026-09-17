# RW-080 §1.3 lowerer slice 20: exact immediate payloads

Status: PROVISIONAL C0 CONSTRUCTION. This is a real bounded Sley lowering
algorithm under the operator development override. It is not RW-110
completion, C1, self-hosting evidence, or runtime authority.

Successor note: `rw-080-lower-instruction-map.md` removes instruction-vector
PSH1 use so the single admitted push row can serve final octet emission.

## Scope and behavior

Every compact operation fact and lowered instruction now carries the complete
canonical SLEYBC02 immediate byte sequence in addition to the tag and two
`UInt64` fields used by the existing Sley validators. The exact bytes cover
all frozen immediate forms:

- `None`: tag only;
- entity: tag plus framed 32-byte entity identity;
- index: tag plus big-endian UInt32;
- field: tag plus the full 32-byte member identity;
- variant: tag, framed definition identity, and full member identity;
- observation: tag plus the full digest; and
- function: tag, framed function identity, type-argument count, and complete
  canonical type encodings.

The bootstrap target excludes observation immediates and generic direct calls,
but the lossless representation no longer prevents their eventual full-profile
encoding. Opcode, immediate-kind, arity, register-reference, and frontier
judgments retain their established order. Semantic resolution of identity and
type payloads belongs to the admitted checker input; the lowerer copies the
canonical bytes after those compact judgments rather than invoking a native
compiler service.

Immediate-free rows must carry the exact canonical `None` bytes as well as
zero compact payloads. A wrong byte sequence therefore returns
`VM_LOWER_IMMEDIATE_MISMATCH` before arity or reference validation.

## Native parity and negative corpus

The eight-family native image now compares complete immediate bytes as well
as every compact field. A dedicated collision case uses two entity identities
with identical final eight bytes and different leading bytes. Their compact
projections are deliberately equal, while Sley returns distinct exact models;
this proves the former projection cannot collapse the eventual image. The
mixed-inventory negative corpus also substitutes entity bytes beneath a
`None` tag and receives the frozen immediate-mismatch code.

All twenty-nine tests in `rw080_lower_scaffold` pass; focused Clippy with
warnings denied is clean.

## Explicit remainder

The Sley closure now holds lossless function metadata, block models, and
instruction immediates. Its successor frees the PSH1 row needed for byte
assembly. Function-body serialization, the transitive callee table, SLEYBC02
header framing, and execution-package assembly remain RW-110 construction
layers. The driver remains RW-120 work. RW-080 and R2 stay provisional pending
the recorded independent acceptance debt.
