# RW-080 §1.3 lowerer slice 32: complete function-body bytes

Status: PROVISIONAL C0 CONSTRUCTION. This is a real bounded Sley lowering and
encoding composition under the operator development override. It is not
RW-110 completion, C1, self-hosting evidence, or runtime authority.

## Scope and behavior

One Sley entry now returns the complete canonical bytes for a lowered function
body. It composes the exact metadata header through block count with the
ordered block-map walker, then closes the final octet vector through V2B1.
The result includes every native `encode_body` field:

- function identity, parameter registers, register types, result type, entry
  slot, and block count;
- every block's slot and parameter registers;
- every instruction's opcode, operand/result registers, and exact extended
  immediate;
- all five terminator families; and
- reachability.

The entry calls the complete semantic lowerer before encoding. Its current
composition then calls the existing header entry, which repeats that validated
lowering once to reuse the already tested header implementation. The first
model supplies the block map. This is deterministic and byte-correct but is a
known construction inefficiency to remove when the header logic is extracted
as a model-to-vector helper. It does not move judgment or serialization to the
host.

The closure uses exactly B2V1, the single UInt8 PSH1 schema, and V2B1.

## Native parity corpus

The native three-block Option-switch function is lowered and encoded by the
composed Sley closure. Its complete body is compared byte for byte with the
native SLEYBC02 `encode_body` layout, covering heterogeneous register types,
all block records, exact immediates, switch/branch/return terminators, and
reachability.

All forty tests in `rw080_lower_scaffold` pass; focused Clippy with warnings
denied is clean.

## Explicit remainder

The function-body encoder does not yet emit the outer `SLEYBC02` magic,
version, root body/callee boundary, or transitive callee table. Those image
layers and execution-package assembly remain, along with removal of the
temporary duplicate lowering pass. The driver remains RW-120 work. RW-080 and
R2 stay provisional pending the recorded independent acceptance debt.
