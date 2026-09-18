# RW-120 lowerer-to-builder handoff — provisional record

Status: PROVISIONAL C0 CONSTRUCTION (2026-09-18, operator development
override `rw075_correction.operator_override_2026_09_07`). This successor
driver passes bytes produced by the Sley lowerer directly into the Sley
package builder during one admitted invocation. It is not whole-toolchain
reconstruction, canonical state root `S`, a fixed point, or an RW-120 pass.

## Runtime handoff

The successor driver retains the same complete 100-function union and calls
the codec, checker, lowerer, and package-builder entries. Its CFG differs from
the preceding tuple-only wrapper:

1. the entry block calls codec, checker, and lowerer;
2. a built-in Result switch branches on the lowerer's typed result;
3. the success block binds the lowerer's `Bytes` case payload as a block
   parameter and uses that exact parameter as package-builder operand zero;
4. the success block rewraps the lower result and returns all four typed
   results; and
5. the error block rewraps the lowerer's `UInt32` failure for both lowerer and
   builder result positions, so package construction does not run after a
   lowering failure.

The wrapper has no independent image-bytes input. Its 74 inputs contain the
exact codec, checker, and lowerer facts plus only the package builder's
remaining 22 inventory/digest inputs. The executable test also inspects the
builder call and proves operand zero is the `Bytes` parameter owned by the
lower-success block.

The resulting graph contains:

| Category | Count |
|---|---:|
| Functions | 101 |
| Parameters | 5,148 |
| Blocks | 1,849 |
| Operations | 4,058 |
| Constants | 699 |
| Adapter imports | 4 |
| Local entry points | 5 |

One admitted invocation produces the exact expected codec and checker values,
the exact native-oracle `SLEYBC02` direct-call image through the lowerer, and
an exact `EXEC_PACKAGE_V2` envelope whose image section is that runtime output.
The executable image is 491,378 bytes, its package digest is
`8b45b177d8271d1b2fc1a2edbe78565ec1609b3d86fbe74eb5371bb0d2f13cdf`,
and the frozen successor gate records 4,058 operations and 147 bridge uses.

## Pinned successor root

- objects/entity bindings: 11,876;
- stored object bytes: 2,971,077;
- object-bundle SHA-256:
  `c40eab81641a843f4983c603cd3483bfcca7a3488658b053576e883deb0d336d`;
- successor component root:
  `4cbcd1eeea202d482895e70ec91f1e0d154c1b7599cc19d60cc6751e61ecfe47`;
- stored root bytes: 784,246;
- stored-root SHA-256:
  `85f94269365956705d3d7206ca2aa9a65099da1d55141fe06e3b60f0ec93bbde`.

Validation commands:

- `cargo test -p sley-vm --test rw120_toolchain_integration integrated_driver_hands_lowered_bytes_to_the_package_builder`
- `cargo test -p sley-vm --test rw120_toolchain_integration`
- `cargo clippy -p sley-vm --test rw120_toolchain_integration -- -D warnings`
- `cargo fmt --all -- --check`

## Remaining semantic boundary

This closes one real byte-level handoff: lowerer output becomes builder input.
The codec and checker still run from independently prepared bounded facts, and
their results do not yet feed the lowerer. The lowerer reconstructs one exact
direct-call fixture rather than the complete toolchain graph; the builder
assembles one package rather than all five toolchain packages. No second build
or fixed-point comparison occurs. Whole-closure decode, semantic checking,
per-entry toolchain reconstruction, typed module-specific build errors, and
self-reproduction remain open.
