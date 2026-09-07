# RW-080 construction contract (corrected, RW-075 repair of AR-04)

Status: CONTRACT ONLY (2026-09-06; AR-04 unification 2026-09-07). No
toolchain graph is constructed in RW-075; no C1 exists. RW-080 stays
BLOCKED until this contract passes the architecture review plus the
RW-075 premium delta re-review returns `R2_ARCHITECTURE_PASS` plus the
aggregate R2 gate reads READY. Then — and only then — may a fresh
`BOOTSTRAP_READY` decision authorize RW-080.

Successor baseline (single current dependency contract): every module
below builds under `BOOTSTRAP_PROFILE_2`
(`fb2d8cc87ee7de68cde8197a77003a417a0062acb6ed087d85f899da1a847459`),
`HOST_ABI_V2`
(`bc564653302a73eb5f998427250a2bb7cd87f5685ef12619bd4ae1f1b2af70d5`),
and `EXEC_PACKAGE_V2`
(`f4958c5e3d57762173b881288b008af17d45b5f07a431fcc442d9eec5770da94`).
`RAW_BLAKE3_V1` (`785205fb...69f72`) is admitted and reachable under
the successor profile (single-shot BLAKE3 over Sley-built preimages,
1 MiB per-call ceiling, typed over-bound refusal) — not "awaiting
wiring". The v1 records (`BOOTSTRAP_PROFILE_1` `4f269150...efd630`,
`HOST_ABI_V1` `e6de00b8...04ecc2`, `EXEC_PACKAGE_V1`
`9e20da24...595d4`) stay byte-identical history; no RW-080 module
builds under them.

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
- Required `BOOTSTRAP_PROFILE_2` capabilities: E1 data (tuples, vectors,
  options, results, comparisons), E2 checked integers (length arithmetic),
  E4 maps (member tables where needed), E5 cells (decode cursor state
  where needed) plus value hashing, E8 bridge (byte/vector conversion for
  I/O framing) — all within the 42 frozen opcodes; no floats, no effects,
  no contracts, no generics.
- Required host ABI primitives: B2V1/V2B1 conversions plus the single
  per-inventory PSH1 row for output construction; `RAW_BLAKE3_V1` for
  envelope digests over Sley-built preimages (admitted under the
  successor profile and reachable here; single-shot only, 1 MiB ceiling).
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
  `EXEC_PACKAGE_V2` envelope (image + sections + digests).
- Input/output canonical types: checked closure in; lowered model
  (bytecode function + callee table) then envelope bytes out; errors are
  typed values in the frozen vocabularies (`LowerErrorCode`,
  `PackageError`, `IMAGE_*` where structural).
- Responsibility boundary: SSMC-to-executable lowering plus deterministic
  assembly of the derived execution image and its package closure. Owns NO
  checking (inputs arrive checked), NO build orchestration, NO admission
  judgment (it assembles evidence; the admission authority judges).
- Required profile/capabilities: full `BOOTSTRAP_PROFILE_2` including E6
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
- Builder faithfulness, staged (AR-07): before presenting any package
  for admission, correspondence between the judged graphs and the
  candidate image must be established by exact byte comparison of a
  reference lowering against the candidate bytes; a mismatch aborts
  with no receipt minted. Who performs that comparison depends on
  stage, and the two stages never mix:
  - C0 seed (now, before C1 exists): the native staged authority
    (`sley-vm::admit_v2_package`) performs judgment plus reference
    re-lowering as seed/oracle evidence. This is the same class as
    conformance builders comparing bytes today — authority/oracle
    work, never host-path execution work — but it IS native semantic
    work, so it is confined to C0 and excluded from clean stages.
  - Post-C1 (clean stages): the Sley driver replicates the comparison
    with Sley-owned lowering evidence, and admission mints only from
    Sley-produced evidence through the reserved
    `SleyAdmissionEvidence` ingress (`sley-vm::admission_authority`,
    currently refusing `SleyEvidenceUnavailable`: no constructor, no
    minting path until C1 exists). Native handling is then
    authenticated/bound structural verification only (byte equality,
    claim binding, table correspondence, digests) — never a semantic
    answer.
  Neither stage permits a hidden seed fallback, cached final image,
  copied build result, or native semantic image construction in a
  clean stage.
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

- Canonical entry point (reserved, not implemented before RW-150):
  `verify_witness(program_closure, witness) -> VerifierResult` where
  `program_closure` is the complete canonical object closure (same
  shape as §1.2 `check_program` input) and `witness` is the exact
  `Bytes` witness artifact named by the RW-150 contract.
- Input/output canonical types: typed closure plus witness bytes in;
  `VerifierResult = Result<VerifierAccept, VerifierError>` out, with
  `VerifierError` covering `WITNESS_MALFORMED | RULE_VIOLATION |
  FLOW_VIOLATION | SINK_VIOLATION | LIMIT` (all typed values, never
  traps). Byte-level handoff: the driver passes the already-lowered
  module images plus witness bytes; the verifier returns the typed
  verdict plus the verified-artifact digest. No other shape is
  accepted.
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

## 1.6 Byte-level module and admission handoffs (AR-04)

Every handoff below is exact bytes plus digests, never prose-level
shapes alone:

- Codec (§1.1) in: SCB1 envelope `Bytes` (strict framing); out:
  canonical object-closure bytes plus `CodecResult` typed value.
  Byte identity: BLAKE3 over the exact envelope preimage image the
  codec constructs as `Bytes` (host hashes only).
- Checker (§1.2) in: canonical object-closure bytes plus the codec's
  output digest; out: mandatory test plan bytes plus `CheckResult`
  typed value. Oracle parity is byte-identical outputs plus identical
  rejection codes.
- Lowerer/builder (§1.3) in: checked closure bytes plus test-plan
  digest; out: `EXEC_PACKAGE_V2` envelope bytes (image + constants +
  layouts + exact import rows + globals + contracts + entry + epoch +
  root + profile + limits) plus section digests. Emitted bytes must
  round-trip through `load_image` and reproduce byte-identically on
  re-lowering.
- Admission ingress: the Sley driver presents the package envelope
  bytes plus Sley-built evidence bytes conforming to the reserved
  `SleyAdmissionEvidence` shape (judged-closure digest, gate counts,
  reference-image digest, complete table digests). The authority
  authenticates and bound-checks that evidence structurally and mints
  the v2 receipt only on exact match. Until C1 exists the ingress
  refuses (`SleyEvidenceUnavailable`); the C0 seed path
  (`admit_v2_package`) is the only minting route and is excluded from
  clean stages.
- Chunk/framing ownership: preimages over the 1 MiB `RHW1` ceiling
  refuse as typed `Err(Index, 2)` at admission (carried constants)
  and execution (computed values). The former `SLEYCHNK1`
  hash-of-chunks construction is retired and carries no identity;
  the domain is reserved and must not be used. A future streaming
  primitive needs its own domain constant and gated implementation —
  never a silent redefinition of an existing domain.
- Host vs staging split: the Sley-callable import registry holds
  exactly B2V1/V2B1/PSH1/RHW1. Structural image loading, package
  execution, digest comparison, and artifact recording are host
  mechanics, not callable imports. Process/file evidence recorders
  are external staging (declared inputs/transport only), never
  Sley-callable imports and never semantic oracles.

## 2. S: the canonical toolchain graph root

S is: the canonical toolchain graph root; the complete canonical object
closure (every entity/object the toolchain needs, content-addressed); the
runtime data/layout closure (constants + type definitions carried as
package sections); declared entry points (the four module mains above);
the build manifest (exact input inventory + expected digests + profile +
epoch + root); and the exact P/H dependencies (`BOOTSTRAP_PROFILE_2`
digest `fb2d8cc87ee7...459`; `HOST_ABI_V2` `bc564653...2af70d5` plus
`EXEC_PACKAGE_V2` `f4958c5e...5770da94` and `RAW_BLAKE3_V1`
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
