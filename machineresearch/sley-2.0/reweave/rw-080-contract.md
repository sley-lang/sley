# RW-080 construction contract (corrected, RW-075 repair of AR-04)

Status: CONTRACT ONLY (2026-09-06). No toolchain graph is constructed in
RW-075; no C1 exists. RW-080 stays BLOCKED until this contract passes the
architecture review plus the RW-075 premium delta re-review returns
`R2_ARCHITECTURE_PASS` plus the aggregate R2 gate reads READY. Then — and
only then — may a fresh `BOOTSTRAP_READY` decision authorize RW-080.

Depends on: RW-060 COMPLETE, RW-070 COMPLETE, RW-075 (this repair).
Minimum gate: graph root, construction manifest, no source DSL.

## 1. First toolchain modules and interfaces

### 1.1 Program/schema codec (Sley program; reference: `sley-scb1` native)

- Canonical entry point: `codec_main(operation, input) -> CodecResult`
  where `operation` is one of `decode_program | encode_program |
  decode_schema | encode_schema` and `input` is the exact `Bytes` or
  canonical object form defined below.
- Input/output canonical types: input `Bytes` (SCB envelope, strict) or
  canonical program/schema object closure (typed values); output
  `Result<CanonicalObject, CodecError>` / `Result<Bytes, CodecError>`
  with `CodecError` covering `TRUNCATED | TRAILING | MALFORMED_TAG |
  SHAPE | COUNT | VERSION | LIMIT` (all typed values, never traps).
- Responsibility boundary: byte-exact SCB decoding/encoding with strict
  rejection and bounded traversal. Owns NO semantic judgment (no
  well-formedness beyond framing, no typechecking, no lowering).
- Required `BOOTSTRAP_PROFILE_1` capabilities: E1 data (tuples, vectors,
  options, results, comparisons), E2 checked integers (length arithmetic),
  E4 maps (member tables where needed), E5 cells (decode cursor state
  where needed) plus value hashing, E8 bridge (byte/vector conversion for
  I/O framing) — all within the 42 frozen opcodes; no floats, no effects,
  no contracts, no generics.
- Required host ABI primitives: B2V1/V2B1 conversions plus the single
  per-inventory PSH1 row for output construction; `RAW_BLAKE3_V1` for
  envelope digests once the RW-080 manifest wires it (admitted in RW-075,
  reachable only here).
- Dependencies: none (leaf module; the driver feeds it bytes).
- Error/result contract: every rejection is a typed `CodecError` value;
  resource exhaustion is `Limit` (never truncation); determinism
  byte-exact across runs.
- Scaffold vs algorithm: scaffold = entry dispatch plus error vocabulary
  with `unimplemented` traps on real inputs; real algorithm = the full
  strict decoder/encoder with the byte/error parity corpus green.
- Reference/oracle comparison: `sley-scb1` native plus the independent
  `sley2-scb1-oracle` (`conformance/scb1/v1` accepted 23 / rejected 26
  plus mutation-value/candidate/result/receipt vectors) — byte-identical
  outputs and identical rejection codes.

### 1.2 Semantic checker and test planner (Sley program; reference: `sley-ssmc` + `sley-check` native)

- Canonical entry point: `check_program(program_closure) ->
  CheckResult` where `program_closure` is the complete canonical object
  closure (functions, parameters, blocks, operations, constants, types,
  adapters, contracts) and `CheckResult = Result<TestPlan,
  SemanticError>` with the frozen S20-210/220/260 vocabularies.
- Input/output canonical types: typed SSMC object closure in, either a
  mandatory test plan (required by REWEAVE for the bootstrap closure) or
  the exact structured `SemanticError` (code + entity + detail refs), out.
- Responsibility boundary: SSMC formation, reference/identity checks, type
  and CFG checking, effect/contract judgments, mandatory test planning.
  Owns NO lowering, NO image assembly, NO build orchestration.
- Required profile: E1/E2/E4 plus E5 cells/hashing (cycle-visited sets),
  E6 direct calls (closure walk), maps/vectors/options/results for
  inventories and diagnostics; bridge for diagnostic byte rendering.
- Host primitives: conversions/push plus raw hash for fingerprint
  preimages the checker constructs as `Bytes` (checker builds
  `SLEYSFP1`/`SLEYVHS1` framings itself; the host only hashes).
- Dependencies: codec (1.1) for inputs; owns its own traversal (no shared
  graph library outside the closure).
- Error/result contract: first-failure deterministic `SemanticError`
  (frozen codes); test plan enumerates mandatory checks with exact
  coverage claims.
- Scaffold vs algorithm: scaffold = entry plus error vocabulary plus one
  admitted trivial program; real = the required semantic corpus green
  (RW-100 gate).
- Oracle: native `sley-ssmc`/`sley-check` plus conformance vectors —
  verdict-identical on the corpus, with independent byte/error parity.

### 1.3 Lowerer and executable-package builder (Sley programs; reference: `sley-vm` lower/execute paths native)

- Canonical entry points: `lower(function_closure) ->
  Result<LoweredModel, LoweringError>` (frozen `LowerErrorCode`
  vocabulary); `build_package(lowered, closure_digests, manifest) ->
  Result<ExecutionPackage, BuildError>` emitting the exact
  `EXEC_PACKAGE_V1` envelope (image + sections + digests).
- Input/output canonical types: checked closure in; lowered model
  (bytecode function + callee table) then envelope bytes out; errors are
  typed values in the frozen vocabularies (`LowerErrorCode`,
  `PackageError`, `IMAGE_*` where structural).
- Responsibility boundary: SSMC-to-executable lowering plus deterministic
  assembly of the derived execution image and its package closure. Owns NO
  checking (inputs arrive checked), NO build orchestration, NO admission
  judgment (it assembles evidence; the admission authority judges).
- Required profile/capabilities: full `BOOTSTRAP_PROFILE_1` including E6
  calls (callee table), bridge for byte emission, raw hash for
  digest/preimage inputs the builder constructs.
- Host primitives: conversions/push (byte emission), raw hash (digests),
  structural image loader for self-verification (load-what-you-built).
- Dependencies: codec (inputs), checker (checked inputs only — the
  lowerer never re-checks; it trusts the checker's evidence bound through
  the driver manifest).
- Error/result contract: lowering failures preserve the frozen codes;
  package assembly failures use `PackageError`; emitted bytes must
  round-trip through `load_image` and reproduce byte-identically on
  re-lowering.
- Scaffold vs algorithm: scaffold = entry plus single-op lowering;
  real = exact profile plus execution conformance (RW-110 gate).
- Oracle: native lowerer plus `SLEYBC02` conformance (byte-identical
  images, identical cache keys and observations on the corpus).

### 1.4 Build/dependency driver (Sley program; reference: Makefile/scripts native driver, permanently outside the clean closure)

- Canonical entry point: `build(manifest) -> BuildResult` where
  `manifest` is the exact toolchain input closure (see S below) and
  `BuildResult = Result<BuiltToolchain, BuildError>` with per-artifact
  digests plus the self-reproduction check.
- Input/output canonical types: build manifest (typed closure inventory +
  entry points + expected digests) in; built toolchain (codec/checker/
  lowerer/builder/driver images + package envelopes + digests +
  fixed-point comparison) out.
- Responsibility boundary: resolves the exact input closure, invokes
  checker/lowerer in order, assembles outputs, and reproduces its own
  executable toolchain (whole-toolchain reconstruction, RW-120 gate).
  Owns NO language judgment itself (it orchestrates module verdicts).
  Builder faithfulness (RW-075 residual retirement): before presenting
  any package for admission, the driver re-lowers the judged graphs with
  the reference lowerer and compares bytes exactly; a mismatch aborts the
  build with no receipt minted. This reference re-lowering comparison is
  authority/oracle work (the same class as conformance builders comparing
  bytes today), never host-path work, and it is what verifies
  graphs-to-image correspondence that structural host checks cannot see
  (e.g. same-opcode rewiring).
- Required profile: maps/vectors/records/variants for the closure graph,
  calls for module invocation, bridge + raw hash for transport and digest
  comparison, cells for build-state threading.
- Host primitives: conversions/push (byte transport), raw hash (digest
  comparison), package loader/executor (runs compiled Sley images),
  process/file evidence recorders (declared inputs/transport only).
- Dependencies: all of 1.1–1.3 plus 1.5's reserved verifier interface
  (as an unimplemented boundary until RW-170).
- Error/result contract: first-failure `BuildError` naming the failed
  module plus its typed error; success carries every artifact digest plus
  the fixed-point equality proof.
- Scaffold vs algorithm: scaffold = manifest parser plus module dispatch
  stubs; real = whole-toolchain reconstruction green.
- Oracle: the native driver scripts plus independent conformance report
  builder — same closure, same digests, same verdicts.

### 1.5 Witness checker/verifier ownership (reserved for later)

- The Witness checker Sley program (RW-170; contracts RW-150) owns Witness
  type rules, data/control-flow integrity analysis, verifier
  admissibility, and sink integrity after WA integration. The verifier/
  discharge Sley implementation (RW-180) owns the semantic implementation
  of approved deterministic discharge profiles. NO native primitive in
  RW-080 performs discharge judgment; the carve-out natives (primitive
  hashing, audited crypto, optional bounded solvers — none adopted) stay
  exact bounded dependencies with imports/owners/versions/semantics/limits/
  tests, and never receive language-owned verdicts. No unimplemented
  Witness semantics may be claimed protective before RW-150/RW-170.

## 2. S: the canonical toolchain graph root

S is: the canonical toolchain graph root; the complete canonical object
closure (every entity/object the toolchain needs, content-addressed); the
runtime data/layout closure (constants + type definitions carried as
package sections); declared entry points (the four module mains above);
the build manifest (exact input inventory + expected digests + profile +
epoch + root); and the exact P/H dependencies (`BOOTSTRAP_PROFILE_1`
digest `4f269150...efd630`; `HOST_ABI_V1` `e6de00b8...04ecc2` plus
`EXEC_PACKAGE_V1` `9e20da24...595d4` and `RAW_BLAKE3_V1`
`785205fb...69f72`; toolchain `1.93.0 minimal`; VM `[1,0,0]` /
lowering profile 2 / lowerer `[2,0,0]`; epoch `08`*32 / root `09`*32 for
closure evidence).

## 3. Construction manifest (every initial entity/object)

Every initial entity/object carries: stable identity (`EntityId`
derived per `sley-id` rules from workspace + candidate nonce + kind +
ordinal — never random, never reused); creation provenance (seed-assembler
record or subsequent Sley-mutation record with parent digests); semantic
owner (codec/checker/lowerer/driver/verifier-reserved, exactly one);
dependency (explicit entity refs, topologically closed); construction
method (`seed-assembled` with the exact seed bytes cited, or
`Sley-mutated` with the producing module + inputs cited); and whether
created by the seed assembler or a subsequent Sley mutation.

## 4. C0's allowed seed-only work

C0 MAY: construct/validate the first canonical toolchain graph S from
explicit typed seed data (recording exact construction provenance per
entity); execute the accepted native reference compiler (the seed
lowering/execution crates) to produce C1; provide independent reference
outputs for comparison (byte/error parity oracles).

The deterministic seed assembler MAY: create typed graph objects from
explicit typed seed data; record exact construction provenance. It MAY NOT
infer semantics, repair inputs, synthesize judgments, or embed compiler
answers beyond the cited seed bytes.

Neither C0 nor the seed assembler may count as C1's later implementation.
Once C1 exists, C1/C2 must themselves perform program/schema
decode/encode, checking, lowering, execution-package construction, and
dependency/build orchestration. Native staging after C1 may: provide
declared inputs; transport bytes; execute the compiled Sley image; compute
allowed primitive hashes (`RAW_BLAKE3_V1` over Sley-built preimages,
SHA-256 artifact mechanics); compare final digests; record
process/file/module evidence. It may NOT provide semantic answers
(check verdicts, lowering choices, package contents, dependency decisions,
or discharge judgments).

## 5. Explicit anti-copy evidence (from the beginning)

No prebaked C2/C3 image exists in S (provenance audit: every entity is
`seed-assembled` or `Sley-mutated`, none `prebaked`). No output-copy
operation counts as a build (the driver must show checker/lowerer
invocations with distinct intermediate digests). Changed toolchain input
must change expected output/behavior (mutation corpus: at least one
semantic change per module with before/after digests). Corrupt/omitted
input must fail (rejection corpus per module). A fake builder returning a
prior image must fail later RW-140 (fixed-point + self-change gates;
the RW-080 manifest records the fake-builder rejection hook but does not
execute it).
