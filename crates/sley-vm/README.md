# sley-vm

Derived register bytecode and, in later packages, the deterministic Sley 2
reference VM. S20-260 implements the restricted `O0-restricted-v1` lowering
profile for all five terminators and the validated Boolean opcode subset.
Bytecode and cache entries are disposable evidence, never canonical SSMC state
or validation authority. S20-270 adds `VM_EXEC_RESTRICTED_V1`: integrated
re-lowering, validated/hashable constant inputs, deterministic Boolean
execution, all terminators, strict fuel/value/output/cancellation limits, and a
canonical observation ID. Unsupported opcode signatures, generics, adapters,
live cancellation, and persistent execution/test reports remain fail-closed.
S20-290 exposes observation rederivation to `sley-conformance`; the VM does not
own report aggregation or persistence.

`VerifiedImage::load_with_mode(..., ExecutionMode::Reference)` explicitly skips
compact-plan preparation and execution while retaining the ordinary image and
request admission checks. `load` keeps its default automatic selection. Prepared
requests inherit the immutable image's mode. Mode is local implementation
configuration and does not change graph, bytecode, cache or observation identity.

Compact execution binds current constants on every run. For at least 16 constant
references and 16 inventory entries, it first checks strict entity ordering and
then uses binary lookup. Smaller, unsorted or duplicate inventories retain linear
first-match lookup. No index or constant value is persisted, no new allocation is
needed for the ordering check, and initial semantic resource refusal still
precedes binding. This optimization changes lookup work, not executed operations,
fuel, value units, return/failure values, output limits or cancellation order.
