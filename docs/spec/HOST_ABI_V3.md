# HOST_ABI_V3

Status: SH2 successor (2026-10-12). Version 3. `HOST_ABI_V1` and
`HOST_ABI_V2` are preserved byte-identical as history.

- Contract: `sley2-host-abi-3`. Rust surface: `sley_vm::host_abi`
  (`HOST_ABI_V3_*`, `BRIDGE_CODE_RHW2/RSH2/BCT1/BSL1/BLN1`,
  `HOST_ABI_V3_LARGE_MAX_BYTES`), `sley_vm::extended` (`BridgeKind`,
  shared `resolve_bridge_entry`, `resolve_push_use`, `is_push_identity`),
  and `sley_vm::host_admission` (pinned-identity admission).
- Profile: `BOOTSTRAP_PROFILE_3` (`8cc5677c...b24ced`).
- Supersedes: `HOST_ABI_V2` (`bc564653...70d5`). Relation: strict superset.

## Import rows

Every row is an `adapter_invoke` (opcode 161) entry named by an `Entity`
immediate, resolved against the carried import inventory. Identity is
`SLY1/BRIDGE/<code>` plus zero padding; the adapter identity equals it;
`abi_version` 1; failure `BuiltinFailure(Index)`; no effects.

| Code | Scope | Request | Response | Failure | Fuel (up front) |
|---|---|---|---|---|---|
| `B2V1` | `Unit` | `Bytes` | `Vector<UInt(8)>` | `Index`, code 2 over 1 MiB | one per byte |
| `V2B1` | `Unit` | `Vector<UInt(8)>` | `Bytes` | `Index`, code 2 over 1 MiB | one per element |
| `PSH1` | `Vector<T>` | `T` | `Vector<T>` | `Index`, code 2 at 2^20 elements | 1 |
| `RHW1` | `Unit` | `Bytes` | `Bytes` (32) | `Index`, code 2 over 1 MiB | `1 + ceil(len/1024)` |
| `RHW2` | `Unit` | `Bytes` | `Bytes` (32), BLAKE3-256 | `Index`, code 2 over 64 MiB | `1 + ceil(len/1024)` |
| `RSH2` | `Unit` | `Bytes` | `Bytes` (32), SHA-256 | `Index`, code 2 over 64 MiB | `1 + ceil(len/1024)` |
| `BCT1` | `Unit` | `Vector<Bytes>` | `Bytes` | `Index`, code 2 over 64 MiB total | `n + 1 + ceil(total/1024)` |
| `BSL1` | `Bytes` | `Tuple(UInt(64), UInt(64))` | `Bytes` | `Index`, code 1 out of range | `1 + ceil(len/1024)` |
| `BLN1` | `Unit` | `Bytes` | `UInt(64)` | none in practice | 1 |

`PSH1` identities carry a big-endian `u32` discriminator in bytes 16..20.
Discriminator 0 is the frozen v1/v2 identity; uses of it keep the v2 rule,
selecting among same-identity rows by exact per-use schemas. A nonzero
discriminator names exactly one carried row, whose request must equal the
pushed element type and whose response must equal the scope type. Under the
profile gates, a nonzero discriminator is v3-only.

## Execution mechanics (no semantic change)

`HOST_ABI_V3` comes with execution-mechanics changes in `sley_vm::execute`.
Every value, termination, instruction count, fuel charge, value-unit charge,
and observation stays identical (the full `sley-vm` suite, including every
frozen vector, passes unchanged, and debug builds assert each derived
value-unit count against the full walk):

- register liveness (`execute/liveness.rs`) lets a dead operand move out of
  the register file instead of being deep-copied;
- ownership-aware arms (`execute/owned.rs`) update vectors and maps in
  place when their operand is dead and unshared, binary-search map keys,
  derive result value units arithmetically from cached operand units, and
  share constants once per execution;
- runtime values are a shared root plus a view path, so tuple, record, and
  payload projections share instead of copying.

## Admission without a compiler (`host_admission`)

`admit_pinned_v3_package(package, pinned)` admits a v3 package only when its
package identity is in an explicit caller-supplied set: the
operator-approved toolchain images, or images the bootstrap record binds to
a pinned toolchain's build. It computes digests, compares identities, mints
the receipt, and runs the structural approval checks. It never calls the
native gate or the reference lowerer. The native admission route
`admit_v3_package` (gate plus reference re-lowering) remains the C0 seed
path.
