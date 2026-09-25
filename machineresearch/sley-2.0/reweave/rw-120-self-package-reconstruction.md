# RW-120 self-package reconstruction — provisional record

Status: PROVISIONAL C0 CONSTRUCTION (2026-09-18, operator development
override `rw075_correction.operator_override_2026_09_07`). The retained
lowerer-to-builder driver now has a qualification in which its Sley lowerer
reconstructs the driver's complete executable image from canonical facts and
its Sley package builder places those runtime-produced bytes into an exact
package envelope during the same admitted invocation. This is not canonical
state root `S`, the required independent fixed-point comparison, or an RW-120
pass.

## Exact self-package result

The qualification lowers the already retained 101-function handoff graph
natively to prepare an oracle and canonical input facts. The admitted handoff
driver then executes its ordinary four-module CFG:

1. the codec and checker run their retained bounded test cases;
2. the fact-fed Sley lowerer consumes the root function and all 100 transitive
   callee facts;
3. the lowerer returns the complete `SLEYBC02` image as `Ok(Bytes)`;
4. the success block passes that exact case payload to package-builder operand
   zero; and
5. the package builder returns an `EXEC_PACKAGE_V2` envelope equal to the
   native oracle for the executing driver and its qualification limits.

The reconstructed image hydrates as the same entry function plus 100 callees.
The output is pinned as:

| Evidence | Exact value |
|---|---:|
| Image bytes | 491,378 |
| Image SHA-256 | `dffdfbbd96585d92a1088f46597ec34bf2271248062a3a84437ba844302552c5` |
| Transitive callees | 100 |
| Package-envelope bytes | 533,671 |
| Envelope SHA-256 | `a73fe4cf621826f490adbb50862d3104a99fba56ba06e1613fd52e1672ec865c` |
| Decoded package digest | `d53bde8d226da2aee57bddeaf093464ef7cbd5f857916dee71aa085c5dcd60e1` |
| VM instructions | 15,480,658 |
| Fuel actions | 74,670,072 |
| Peak value units | 6,027,165,516,738 |

An independently shaped checker-package qualification exercises the same
arbitrary reconstruction path with an eight-function target. It produces a
35,032-byte image with seven callees and a 40,280-byte envelope. Its pinned
image, envelope, and package digests are respectively
`cea90d9e204e1f5f3f6fc0e7a6d7edd3a639e924d02e8c2c794c9500a9f09bd2`,
`08d9db121febb04bdf79e24e201d6e5d402ac8246f69e79c045f3d6e15e2bbed`,
and `c3ec1743ed10de737d1eb1d594255783c6ffc9df76771d8fae3e99352778467c`.
It consumes 1,146,281 instructions and 5,491,836 fuel actions, with
35,065,761,207 peak value units under the same qualification limits.

Both qualifications are ignored in the default test run because the complete
case is an explicit release-mode measurement. Their assertions remain in the
integration target and pin every value above.

## Runtime scaling repair

The qualification initially exposed quadratic host copying in the general VM
executor. The repair does not add a toolchain-specific opcode or host callback:

- runtime values share immutable roots and move a value only when its current
  block definition is dead after the instruction or used once on the selected
  outgoing edge;
- wrapper payload views retain an exact cached value-unit count;
- the frozen `PSH1` bridge moves a dead accumulator and derives the new result
  charge arithmetically; and
- ordinary extended instructions borrow operands, cloning only data that
  becomes part of the result.

The active VM unit suite preserves exact termination, fuel, instruction, and
resource-accounting checks. On the checker qualification, the final borrowed
operand and loop-safe edge repair reduced release execution from 34.8 seconds
to 3.9 seconds. The reconstructed image stayed exact; the final envelope and
execution counters are pinned under the recorded qualification limits.

Validation commands:

- `cargo test --release -p sley-vm --test rw120_toolchain_integration integrated_driver_reconstructs_the_checker_package -- --ignored --nocapture`
- `cargo test --release -p sley-vm --test rw120_toolchain_integration integrated_driver_reconstructs_its_complete_executable_package -- --ignored --nocapture`
- `cargo test -p sley-vm --lib`
- `cargo test -p sley-vm --test rw120_toolchain_integration`
- `cargo clippy -p sley-vm --lib --test rw120_toolchain_integration -- -D warnings`
- `cargo fmt --all -- --check`

## Remaining semantic boundary

The canonical facts are still prepared by the host from an already lowered
graph. Codec output does not feed the checker, and checker output does not
derive the lowerer facts. The result reconstructs the complete executable
closure and its package, but it does not reconstruct the state-root object
graph, establish canonical `S`, perform the required second independent build,
or compare two separately produced roots. Those boundaries keep fixed point,
RW-120 completion, succession, and R2 release status open.
