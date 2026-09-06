# BOOTSTRAP_PROFILE_1

Status: frozen (RW-050 slice 2, 2026-09-06). Version 1.

- Profile digest: `4f2691504b5c756eae1f5ef01e6e998cc4cd628d4b524b038b10d583bfefd630`
  (raw-byte SHA-256 of `conformance/bootstrap-profile/v1/profile.json`;
  the manifest stage P binds this digest, and
  `scripts/check_bootstrap_profile_1.py` verifies the binding on every
  `make quick`).
- Contract: `sley2-bootstrap-profile-1`. Machine record:
  `conformance/bootstrap-profile/v1/profile.json` (authoritative for
  values); this document (authoritative for rationale and rules).

## What this is

`BOOTSTRAP_PROFILE_1` is the exact semantic/execution subset sufficient to
build the Sley toolchain: the REWEAVE §10.2 closure (canonical
program/schema processing, language-level checking in the compiler's
domain, lowering, executable-image construction, the integrated
destination's Witness analysis, and the Sley-assigned build driver) as
executable Sley programs. It is a static subset of `EXTENDED_V1`
(`VM_EXTENDED_OPCODE_PROFILE_V1.md`), judged by the production gate
`sley_vm::bootstrap::judge_bootstrap_profile`. It is not a new execution
semantics: lowering and execution under `EXTENDED_V1` are unchanged, and
no execution entry calls the gate.

It is distinct from the final GA language/profile (complete eventual
Machine Genesis/WITNESS release surface, later packages) and from the
RW-070 ABI/image freeze (later artifact/import/runtime boundary
qualification). Freezing this profile freezes no external host ABI.

## Bindings

- Schema epoch `08`*32, state root `09`*32 (closure evidence binding,
  same convention as the vm-extended vectors).
- VM: `vm_version [1,0,0]`, `lowering_profile 2`, `lowerer_version
  [2,0,0]`, bytecode `SLEYBC02`, cache profile `EXTENDED_V1`.

## Permitted opcodes (42)

E1 data with the base Boolean operations (1, 16, 17, 32–35, 96–104,
128–131), E2 checked integers (64–71), E4 records/variants/maps (18–21,
36–40), E5 cells and value hashing (176–178, 192), E6 direct calls (112).
Opcode 161 is admitted only through genuine bridge resolution, never by
tag membership. Excluded with rationale: E3 floats 80–85 (the closure is
integer/byte/map-indexed; exclusion keeps host float-environment
dependence out of the bootstrap determinism claim); E7a 144 (CondBranch +
Trap substitutes); E7 145/160/162 (full MG obligation, G-2/RW-160); E5
193/194 (constant_ref and direct calls cover the needs; no dynamic
dispatch).

## Control flow

Terminators Return/Branch/CondBranch/VariantSwitch/Trap (tags 1–5); trap
codes Unreachable/ResourceExhausted/AdapterContractViolation/
InternalInvariant (tags 1–4). Canonical iteration is the CFG backedge
loop (Form A, `cfg-backedge-loop-form-A`) with block-parameter or cell state; recursion is excluded
(recursive call cycles refuse with `VM_LOWER_PROFILE_UNSUPPORTED`);
multi-function calls require an acyclic graph, and admission covers
exactly the reached closure (inventory functions no call reaches cannot
ride an admission — callers narrow inventories first, as lowering
does). The 256-live-frame VM
ceiling stays as defense in depth. Rationale: multi-function structure
is required, recursion is not independently necessary, and only
iteration handles arbitrarily deep structures (RW-040 loop-form mapping;
`cond-drain-loop` cell evidence retained).

## Types

Permitted: Unit, Bool, SInt/UInt, Bytes, Text, Tuple, Named, Vector,
OrderedMap, Option, Result, BuiltinFailure, and LocalCell inside
executions only (operation results and block parameters for in-execution
state threading; never in function parameters, function results, or
constants, which have
no cell form). Excluded: F32/F64, AdapterHandle, CapabilityToken,
FunctionRef, TypeParameter (no generic specialization).

## Permitted native imports (exact, default-deny)

Only the three reviewed pure primitives, as genuine epoch-1
`AdapterImport` rows (identity `SLY1/BRIDGE/<code>` zero-padded to 32
bytes, equal adapter identity, `abi_version` 1, empty effect list):
`B2V1` (Unit, Bytes → Vector⟨UInt(8)⟩), `V2B1` (Unit, Vector⟨UInt(8)⟩ →
Bytes), `PSH1` (Vector⟨T⟩, T → Vector⟨T⟩, any bootstrap T).
Push instantiates by monomorphization (landed: the UInt(8)
representative and the Unit trail). Unknown identities remain denied and
imply no extensible registry. One resolution authority
(`resolve_bridge_entry`) serves lowering, execution, surcharge, and gate
admission. Functions declare no effects; no `AdapterCall` judgment is
delegated anywhere.

## Failure registry

Arithmetic 1–3, Index 1–2, DuplicateKey 1, ContractViolation 1,
Capability 1–4 (S20-210 closed kinds; codes per kind, including the
bridge-capacity code `Index(2)`).

## Resource ceilings and budgets

Code-pinned: 256 live frames, 1,048,576 cells, `BRIDGE_MAX_ITEMS`
1,048,576, tuple arity 64, 1,000,000 constant elements, 16,777,216
constant payload bytes, 262,144 inputs, 67,108,864 input value units and
observation preimage bytes, 1M registers, 4096 lowered blocks, 100M
lowering work. The S20-210 constant ceiling binds direct vector inputs below the bridge cap: V2B1-at-cap and PSH1-at-max are reachable only through single-execution chains. V2B1-over-cap is unreachable by construction (past-cap vectors are unconstructible: inputs capped, B2V1 caps its output, push refuses at the cap), recorded here as defense in depth, not tested through an impossible input. Reference budgets for closure
evidence: 100,000 instructions, 10,000,000 fuel, 100M value units, 10M
output units, no cancellation point (measured maxima over the 20
vectors: 34 instructions, 124 fuel). A closure vector terminating on
resources under these budgets is a profile violation.

## Closure evidence

20 vectors in `conformance/bootstrap-profile/v1/accepted.json` (SHA-256
`878f3009690af8a14223667e497d4579a74e1345eea39cf8e75870fdbc7735ad`),
every one gate-admitted pre-emission: byte round-trip, push-loop growth
(5 and 0), symbol table (hit/miss), set-as-map, record/variant walk,
multi-function pass (value/error), bounds-checked traversal (hit/miss),
content-hash chain, exhaustive switch (4), graph worklist (chain/cycle),
image emission (3/0). Unsupported opcodes, types, imports, effects, and
recursive cycles refuse before execution (gate tests plus the
adversarial lanes). Cache-key inputs for exact reproducibility are
listed in the machine record.

## Sufficiency

The bounded paper/executable check
(`machineresearch/sley-2.0/reweave/rw-050-sufficiency.md`) maps every
§10.2 responsibility to profile capabilities and answers the eleven
sufficiency questions without implementing the compiler. No missing
capability was found; no convenience primitive was added.
