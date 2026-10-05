# Executed workbench capabilities

This matrix addresses [audit #21 item 5](https://github.com/sley-lang/sley/issues/21).
It measures the published **2.0.6 Linux x86_64 `sley-agent`**, not an unqualified
claim about every semantic entity, input or execution route. Representation,
validation, lowering, advisory execution, native test evidence and transaction
admission are separate observations.

Start with the [recorded executable matrix](examples/capability-2.0.6/MATRIX.md).
Each row links its actual verdicts, calls and commit/refusal results. The
[runner](examples/capability_matrix.py), inputs, raw transcript and hashes are
included. The [stock-reorder application](STOCK_REORDER.md) combines supported
features into a CSV workflow with tests, a portable package and maintenance.

## What was exercised

| Surface | Observed boundary |
|---|---|
| Checked integers and `Result` | Correct values and an overflow result; this does not mean integer overflow silently wraps. |
| Boolean control flow | Both branch outcomes. |
| Vectors and loops | Empty and nonempty folds, explicit loop state and checked multiplication. |
| Named records and variants | Field access, empty case and payload case. |
| Text | Exact equality; no file, network or terminal access is implied. |
| Ordered maps | Present and missing lookup results. |
| Floating point | Two finite additions; this is not exhaustive floating-point qualification. |
| Local cells | Local creation, update and read; no external persistence. |
| Direct calls | Calling a pure helper and returning its result. |
| A declared type parameter | Canonical raw authoring, `Valid`, and an untested commit succeed; calling it refuses at lowering with `VM_LOWER_PROFILE_UNSUPPORTED`. The parameter is unused; executable polymorphism is not demonstrated. |
| Effect and contract definitions | The canonical schema has these entity kinds, but this workbench's raw reader refuses kinds 11 and 13 before validating a body. Later stages are **not reached**, not reported as successful or kernel-rejected. |

Successful calls establish that lowering and execution occurred for their exact
candidate/input. The runner does not expose or time a separate lowering command.
Ten feature rows each run two finite cases. Their direct untested commits are
observed separately from combined code-and-test candidates.

The canonical schema and validator support a broader model than this authoring
path. The released [raw reader](https://github.com/sley-lang/sley/blob/4c0d97c55a3c41d453777fb740b2f14b8f89a2d0/crates/sley-agent/src/raw.rs)
accepts entity kinds 3–9 and 14. The released
[VM lowerer](https://github.com/sley-lang/sley/blob/4c0d97c55a3c41d453777fb740b2f14b8f89a2d0/crates/sley-vm/src/lower.rs)
refuses functions declaring type parameters, effects or contracts, including
such callees. These are source facts, distinguished from the executed probes.
The effect/contract probes do not construct a valid effectful program or exercise
a native adapter. Unsupported raw authoring is not evidence that the canonical
model lacks the feature.

## Three different test/commit outcomes

The executable rows record all three paths:

1. A new function with **no selected tests** validates, executes on the example
   inputs and commits. This is an untested transaction, regardless of those
   advisory calls.
2. Adding TestCase definitions to **unchanged accepted code** runs the newly
   authored cases in the workbench. The candidate verdict selects **zero** tests,
   and its commit succeeds. This is not native test evidence or atomic admission
   of a combined tested code change.
3. In another new workspace, a candidate **creates code and its TestCases
   together**. The verdict selects two tests per row, and advisory execution
   matches both expectations. Commit refuses with `TXN_TEST_EVIDENCE_UNSUPPORTED`;
   accepted repository files remain unchanged.

Thus a populated `tests` output is not sufficient to infer native evidence or
even a nonzero kernel-selected test count. Read the verdict and transaction
outcome. The application likewise selects 17 canonical tests and deliberately
records the tested-commit refusal. No native supervisor was installed or invoked.
Authoritative tested admission remains audit item 4.

## Reproduce

Download the agent from the [2.0.6 release](https://github.com/sley-lang/sley/releases/tag/v2.0.6)
and verify the release checksums. The archive SHA-256 is
`90e072569da7d6fc340ba850803b8e13cc0a608dc831f6ddb4c01c7996f4a054`;
both runners also require binary SHA-256
`7737c4f09c7236555155c25a967b4d52a8fb8f815464e7bcb33935e12e2728c5`.
Use Python 3.9 or newer. Output directories must be new.

```sh
python3 -B docs/examples/capability_matrix.py \
  --agent /path/to/sley-agent --output ./capability-run
python3 -B docs/examples/verify_stock_reorder.py \
  --agent /path/to/sley-agent --output ./application-check
```

The matrix imports the adjacent `stock_reorder.py` for process, hashing and
repository helpers; copy both together when running outside a checkout. The
application verifier needs that helper too. No Rust build, provider, account,
network or supervisor is required for these demonstrations. Each Sley command
runs with a minimal environment and a 60-second process timeout; this host
timeout is not native resource evidence.

The recorded runs used copies of these scripts and the published binary in a
new directory outside the checkout. All 13 rows passed their asserted positive
or refusal boundary. This is existing-host reproduction, not a fresh OS or an
independent adopter. The unsigned hash inventories identify bytes; they are not
independent attestations. Input/output paths in recorded command arguments are
relative to the run directory.

The matrix and stock-reorder example are new. The vector fold adapts the
published AF1-X quick-reference example; the runners reuse existing workbench
contracts. No kernel, VM, protocol, native-admission or release behavior changes.
