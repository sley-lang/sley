<!-- Tracked copy of the operator's amendment, source
/home/dev/Downloads/Sley-ZJX-Readiness.md (SHA-256
dde3ec640bb7eec76409d44e6aea2ed362135d797126b24c57641aee03178af0), filed
2026-09-15 under docs/audits with exactly one edit: section 2's sentence
naming the origin repository is reworded because tracked files may not
spell that name (scripts/check_clean_room_boundary.py). Every other byte is
the operator's. The closeout that implements it is
S20_ZJX_TRANSPORT_READINESS_CLOSEOUT.md in this directory. -->

# Sley 2.0.0 — ZJX Transport Readiness Amendment

**Document ID:** S20-ZJX-READINESS  
**Revision:** 2, audited replacement draft  
**Date:** 2026-09-15  
**Target:** Pre-release readiness assessment and, only where justified, bounded internal repairs to Sley 2.0.0  
**ZJX release, edition, profile, ABI, and packaging:** Unselected  
**Status:** Proposed for explicit activation; implementation and release qualification are not asserted  
**Supersession:** Replaces the previous 40-section “Pre-Release ZJX Integration Scaffolding” draft in full. Do not combine the two documents.

## 1. Objective and governing decision

Prove that a future optional ZJX transport adapter can deliver an exact, supported Sley artifact to Sley's existing authoritative import path without redefining canonical bytes, identities, schema interpretation, validation, policy, repository acceptance, or execution.

Prefer evidence that the necessary boundary already exists. Add production scaffolding only to correct a demonstrated obstruction. A documentation-and-test-only closeout is a successful outcome, not incomplete implementation.

This amendment does not select the eventual ZJX implementation or make ZJX part of Sley 2.0.0. It preserves an additive integration route. It does not promise that every conceivable future ZJX feature can be added without changes elsewhere.

**Governing principle: Preserve Sley's canonical contracts; make transport replaceable outside them.**

## 2. Evidence basis and activation baseline — ZR-01

The design audit inspected selected files in the private origin repository (its current and former remote names are withheld here under the clean-room sentinel rule; the operator's original names them) at commit:

`70283ce1932d4182f206f0d09e6129af3976a4c2`

That is an audit observation, not an instruction to reset or switch the working repository to that commit. It does not capture unpushed work. The audit was a contract/code reading, not execution of the implementation's tests.

Before editing, the implementation owner MUST discover and record the actual authoritative checkout, branch, HEAD, working-tree changes, concurrent worktrees, applicable instructions, active specifications/amendments, release-candidate identity, and current evidence dossier. Preserve unrelated and uncommitted work. Do not reset, clean, overwrite, rebase, or interfere with another active writer to obtain a convenient baseline.

Record the relevant toolchain, dependency-lock digest, release feature set, canonical contract and fixture digests, and current qualification states. Identify which artifact profiles and production call paths are actually supported by that baseline. Do not infer GA completeness from a historical README status paragraph or a conformance helper's name.

Every requirement below MUST have an evidence row containing its ID, owning contract, actual implementation symbol, existing test, gap disposition, relevant commit, and reviewer disposition. Reuse the established obligation register, or attach one amendment matrix referenced by it; do not create a competing release authority.

Permitted initial dispositions are `EXISTING_PROVEN`, `LOCAL_REPAIR_PROPOSED`, and `BLOCKED`. A claimed existing seam needs a production symbol and a test or executable witness, not an architectural diagram alone.

## 3. Authority and controlled scope — ZR-02

Explicit operator instructions and applicable repository instructions govern activation. The active Sley 2.0 master specification, GOLD procedure, accepted amendments, and owning contracts retain their existing authority. This amendment changes readiness procedure only; it does not silently supersede product semantics or frozen compatibility contracts.

An instruction to audit or write this document does not itself authorize repository modification or publication. Once explicitly activated for implementation, the scope is discovery, traceability, architecture documentation, focused tests, and independently reviewed internal repairs within the boundaries below.

Before a production repair, record the exact obstruction, proposed files/symbols, compatibility impact, resource impact, affected evidence, and rollback boundary. Obtain an independent review under the established project workflow. If that review identifies a frozen-contract or public-behavior change, STOP that repair and obtain explicit scope authorization.

Do not activate dormant future specifications merely because they mention ZJX or artifact integration. Preserve existing permissions and restrictions for commits, pushes, tags, release minting, distribution, publication, external calls, and spend. This amendment grants no additional authority for those operations.

## 4. Non-goals and prohibited expansion — ZR-03

This amendment MUST NOT add or choose a ZJX dependency, executable invocation, codec, FFI boundary, subprocess backend, edition, license tier, wire format, dictionary scheme, encryption/signature mechanism, or compression profile.

It MUST NOT introduce a functioning `.sley.zjx` format, ZJX file magic/MIME registration, `sley.zjx.envelope.v1`, protocol operation, user-facing command, package registry, context-bundle system, compiler-cache format, default compression, generalized plugin framework, or dynamic provider loading.

It MUST NOT add a Sley source language, parser, source AST, canonical text representation, or legacy compatibility requirement. SSMC1 remains the machine-native program representation; SCB1 remains the existing canonical encoding; SMP1 remains the existing machine interface.

Do not mandate new crates, trait names, artifact descriptors, extension maps, generic error enums, or provider registries simply to satisfy a suggested abstraction. Existing functions, typed IDs, byte inputs, and owning modules can satisfy the requirement.

## 5. Required architecture and dependency direction — ZR-04

The following is conceptual data flow, not a new wire format or API:

```text
Existing native Sley artifact bytes ---------------------+
                                                       |
Future optional ZJX transport                          |
    -> bounded reconstruction of exact Sley bytes -----+
                                                       v
                         existing authoritative import/preflight
                                                       v
                          existing persistence/acceptance rules
                                                       v
                              existing checked state and execution
```

The future composition layer may depend on the selected ZJX backend and Sley artifact APIs. Sley semantic/identity/schema/transaction/runtime authorities MUST NOT depend on that composition layer or on ZJX.

Transport reconstruction produces input for validation. It does not produce a trust grant. A transport adapter MUST NOT bypass an owning importer by directly constructing a nominally accepted graph, root, pack, or receipt and passing it to execution or persistence.

Repository paths, stores, protected policy context, and execution requests retain their legitimate roles. This amendment does not replace Sley's existing execution context with an oversimplified `run(graph)` API.

The readiness claim is satisfied by proving at least one lossless, optional integration path for the supported artifact profiles. It does not require genericizing every storage, query, cache, or runtime ingress.

## 6. Reuse existing artifact contracts — ZR-05

Inspect and reuse the current owning contracts, including where applicable:

- S20-170 Repository Pack v1: exact root/object exchange;
- S20-540 Repository Exchange v1: repository reconstruction including its explicitly defined ancestry and branch/head facts;
- any additional active artifact contract actually selected by the scope review.

Keep those scopes distinct. A root/object pack is not a complete repository clone. An exchange is not automatically an executable package or context capsule. Do not silently promote one to another.

The audited S20-170 contract already permits future ZJX transport of an exact pack while preserving that pack's bytes and semantics. Its frozen compression profile is `0` (`none`). The audited S20-540 contract also freezes profile `0`. A future outer transport can compress the exact stored bytes while leaving the inner profile unchanged.

This amendment MUST NOT enable a formerly rejected nonzero profile in either existing contract, change reserved fields, accept signatures previously rejected, or reinterpret an existing epoch. A later native compressed contract/profile requires its own explicit versioning and compatibility decision.

The eventual integration may target packs, exchanges, another already-supported artifact, or a separately specified future representation. This amendment selects none of those as a permanent product default.

## 7. Identity and exact preservation — ZR-06

Reuse Sley's existing identity owners, digest algorithms, domains, preimages, and independent vectors. Do not introduce a generic `graph_hash` or leave established identity algorithms implementation-defined.

At minimum, the architecture review MUST distinguish stable entity identity, immutable object identity, state identity, repository ancestry/receipt identity, and complete artifact identity. The current names include `EntityId`, `ObjectId`, `StateRoot`, `TransactionId`, `ReceiptId`, `SchemaEpochId`, `RepositoryPackId`, and `RepositoryExchangeId`; relevance depends on the selected artifact contract.

A future transport's own digest is a separate identity. It MUST NOT substitute for any of those Sley identities. A matching StateRoot alone does not establish equal repository ancestry or equal exchange artifacts. A semantic fingerprint is not a replacement for a content address.

For the readiness witness, the minimum invariant is exact recovery of the original complete supported artifact bytes, including their existing framing and digest trailers. All embedded IDs, bytes, ordering, epochs, closure facts, and applicable repository facts remain identical.

```text
reconstruct(transport(native_bytes)) == native_bytes
owning_import(reconstructed_bytes) == owning_import(native_bytes)
```

The second equality means the same contract-defined accepted facts or the same owning rejection outcome under the same policy and resource context. It does not require incidental timings, allocation addresses, or diagnostic wall times to match.

Existing canonical metadata stays in its existing digest preimages. Do not remove a field from identity merely because its name sounds like provenance, a label, or metadata. Two programs that behave similarly need not have equal canonical identities.

## 8. Byte-oriented boundary without mandatory streaming — ZR-07

Demonstrate a supported import route whose artifact input is bytes or a bounded reader, rather than an input artifact pathname interpreted by the semantic kernel. Filesystem paths may still be necessary for the destination store and its durability model.

An existing bounded `&[u8]` API satisfies this requirement. Fully incremental decoding, async I/O, zero-copy loading, random access, and streaming exports are not required for this amendment.

A future transport may reconstruct into a bounded buffer before calling the owning importer. Its eventual implementation must account for the simultaneously live transport buffer, decompressed bytes, parser allocations, and staging resources. Merely calling an interface a stream does not prove bounded memory.

If an internal reader wrapper is justified now, it MUST handle short reads, premature EOF, exact framing, cancellation where the owning operation supports it, and I/O failure without partial success. The implementation must show where each bound is enforced. Do not introduce network access, implicit temporary-file spill, arbitrary path extraction, or hidden backend discovery to prove the seam.

## 9. Schema, format selection, and version independence — ZR-08

Preserve the existing exact selection of contract tag, schema epoch, supported version, and preserved decoder. A recognized header is not proof of validity. Shared SCB1 magic is not sufficient to distinguish every Sley artifact contract.

An explicit expected artifact kind/version constrains decoding; incompatible bytes must not trigger fallback through unrelated decoders. Unsupported or ambiguous input fails at its owning boundary. Filename suffixes, incoming MIME labels, and producer strings confer no authority.

No new automatic probing or provider registry is required. A future adapter can be explicitly selected by a composition layer. Any subsequent automatic recognition scheme requires bounded, deterministic, fail-closed selection in its own specification.

Sley release numbers, SCB1 format version, schema epochs, inner artifact contract versions, future outer format/profile versions, and ZJX implementation versions MUST remain conceptually distinct. “Use the latest ZJX” is not a compatibility rule. This amendment registers none of those future versions.

## 10. Metadata and authority separation — ZR-09

Retain SCB1's existing closed-field and extension-allowlist rules. The audited SCB1 contract rejects unknown record fields and unknown extension tuples. This amendment MUST NOT add an arbitrary extension map, ignore unknown canonical fields, allow opaque canonical data under an unregistered namespace, or weaken a closed grammar.

Future transport-only metadata can live outside the unchanged inner Sley artifact in a separately specified envelope. That future specification must define required-versus-optional fields, bounded handling, duplicate handling, preservation, identity coverage, and rejection policy. It is not necessary to implement those fields now.

Artifact features such as “contains a root” or “compressed” are not authorization capabilities. An artifact's declared features, provenance, receipts, signature presence, or producer claims MUST NOT grant filesystem access, network access, execution, ref movement, trusted-validator selection, a larger resource budget, or policy modification.

Existing policy/capability owners remain authoritative. Integrity is not authenticity; neither is permission to execute. Unverified metadata must not be presented as independently established fact.

## 11. Verification, errors, and atomicity — ZR-10

All reconstructed bytes MUST pass the same existing structural, digest, schema, closure, object, and other contract-required checks as native bytes. Transport success cannot substitute for a Sley check. Noncanonical inputs must be rejected, not sorted, normalized, repaired, or silently migrated.

Preserve each artifact's actual verification scope. A structural inspection does not imply semantic acceptance; canonical validity does not imply execution authorization; an imported receipt does not automatically authorize changing a local head. Do not add a new verifier that duplicates Sley's semantic authority.

Inspection and verification documentation must distinguish checked, failed, not checked, and inapplicable stages. This may be expressed using existing representations; no new public result enum is mandated. An unchecked stage must never become a default success boolean.

Preserve exact existing error symbols, phase ordering, and relevant precedence for existing inputs. Outer transport failures belong to the future outer layer. They must not erase an inner `SCB_*`, `PACK_*`, `EXCHANGE_*`, schema, store, policy, or transaction failure. New public error codes require owner approval and compatibility review; do not invent them here.

Respect the existing preflight/persistence split. No future wrapper may expose successful import, usable accepted state, or a moved ref while its required terminal framing/integrity checks remain incomplete. The simplest admissible design fully reconstructs and finalizes the bounded transport before invoking persistence-capable native import.

On invalid input, preserve each owner's no-write/no-acceptance guarantee. On persistence interruption, preserve its documented crash model. Existing contracts may permit unreachable immutable objects or staged data after an I/O failure; this amendment does not falsely require a global filesystem rollback. No partial accepted state is allowed. Existing lock, GC, retry, recovery, and compare-and-swap rules remain in force.

## 12. Resource contracts — ZR-11

Inventory existing validity-affecting maxima and host/policy resource ceilings. Record their owner, preflight/allocation enforcement point, aggregate accounting, and failure code. Preserve existing frozen values and behavior.

A future outer decoder must operate within an explicit resource policy covering encoded input, reconstructed output, allocations, metadata, nested objects/frames, and work/cancellation as applicable. Incoming lengths are claims to validate, not allocation authority. Output limits must be enforced while output is produced, not only after materialization.

Do not weaken inner limits, multiply them silently across nested layers, or reset an aggregate budget for each nested value. Do not require an unchosen ZJX backend to fit an invented budget today. Instead, document which ceiling applies at the Sley boundary and which transport budgets remain for the future integration specification.

Missing validity-affecting safeguards in existing production code are findings under their current owners, not an excuse for an unreviewed late format change. No real decompression is implemented in this amendment.

## 13. Concrete integration witness — ZR-12

Provide an executable witness using supported fixtures and the actual production import path. A diagram, empty trait, alternate test-only validator, or mock decoder that bypasses production checks cannot satisfy readiness.

Where a profile has a supported production exporter, exercise that exporter as well as an independently pinned native fixture; prove the future adapter can obtain artifact bytes without duplicating serialization. Exercise the same valid native artifact by direct byte input and by an explicitly test-only framing or segmented-input helper that reconstructs the identical bytes. The helper must not pretend to implement ZJX, use a released ZJX format ID, register a production format, or escape into the distributed feature set.

For each artifact profile claimed ready, compare exact reconstructed bytes, applicable IDs, accepted roots, object contents, and applicable repository facts. Exercise its actual verifier/context rather than relying solely on a restricted conformance fixture. If the real production path cannot accept the selected profile, report that limit rather than broadening the profile.

Also carry malformed native fixtures through the same helper. Once outer reconstruction succeeds, the owning native rejection must remain unchanged. Test an outer failure after a complete-looking inner payload to prove there is no early persistence or accepted result.

An independent reviewer must identify the future adapter's exact insertion point and explain why it neither becomes a kernel dependency nor bypasses validation. This establishes a feasible integration path, not ZJX interoperability, compression quality, or performance.

## 14. Minimum validation matrix

Reuse existing tests wherever they already establish the requirement. Add tests only for uncovered obligations. Every row needs an exact command, baseline/candidate identity, result, log, and evidence reference. A row may be inapplicable only with a precise profile-based explanation and independent acceptance.

| Test ID | Required observation | Requirements |
|---|---|---|
| ZT-01 | Existing positive fixtures and supported production exports retain exact bytes, IDs, epoch selection, and accepted facts. | ZR-05, ZR-06 |
| ZT-02 | Direct input and test-only reconstructed input converge through the same production importer. | ZR-04, ZR-07, ZR-12 |
| ZT-03 | Existing malformed/noncanonical fixtures retain owning error symbols; no normalization occurs. | ZR-08, ZR-10 |
| ZT-04 | Truncation, extra trailing bytes, wrong contract/version/epoch, and unsupported compression still reject. | ZR-05, ZR-08, ZR-10 |
| ZT-05 | Missing, surplus, duplicate, reordered, and substituted contents retain exact closure rejection. | ZR-05, ZR-10 |
| ZT-06 | Unknown canonical fields/extensions remain rejected; annotation claims grant no authority. | ZR-09 |
| ZT-07 | Invalid preflight leaves state untouched; persistence faults preserve the owning recovery model. | ZR-10 |
| ZT-08 | Limits are enforced at the correct layer and before excess allocation/output; nested accounting is documented. | ZR-07, ZR-11 |
| ZT-09 | Test-helper outer failure, including late finalization failure, cannot cause early import/promotion/success. | ZR-10, ZR-12 |
| ZT-10 | Destination path/incidental outer annotations do not change exact inner identity; canonical metadata remains bound. | ZR-06, ZR-09 |
| ZT-11 | Dependency/build checks show no ZJX/backend/plugin/network requirement in default or release features. | ZR-03, ZR-04 |
| ZT-12 | Existing machine protocol, CLI, diagnostic, and execution behavior remain compatible. | ZR-03, ZR-10, ZR-13 |
| ZT-13 | Independent canonical vectors/oracle results remain unchanged; golden data was not regenerated to conceal drift. | ZR-06, ZR-13 |
| ZT-14 | Qualification impact, artifacts, performance evidence, and independent review are bound to the actual candidate. | ZR-01, ZR-02, ZR-13, ZR-14 |

For existing bounded-reader code, cover short-read segmentation and I/O errors at that production boundary. For the test-only helper, those tests prove the helper's behavior only. Do not claim production streaming support from test infrastructure.

Run focused checks first, then the established cross-lane and release checks required by the affected files and contracts. Preserve failing baselines, negative results, skipped checks, and genuine authority/environment blocks. Do not remove failures from evidence.

## 15. Compatibility and performance — ZR-13

Freeze the existing accepted and rejected behavior before a repair. Preserve canonical bytes, domains/preimages, tags, epochs, identity vectors, allowed profile values, closed grammars, validity limits, protocol operations, public error codes, and their governing precedence.

Refactoring must not route every graph/VM operation through a provider dispatch. The transport boundary belongs at ingress/egress, not in semantic hot paths.

Measure relevant pack/exchange import/export, startup where touched, and peak memory against the preserved baseline under the existing performance methodology and investigation thresholds. Do not replace a measurable gate with “no material overhead,” or invent a universal numerical tolerance here. A docs/test-only change may demonstrate executable non-effect rather than requiring an irrelevant performance campaign.

Any fixture or oracle change needs independent justification from its owning contract. It cannot be approved merely because the implementation changed. A necessary public compatibility or validity change leaves this amendment's default repair scope and requires an explicit decision.

## 16. Release evidence and requalification — ZR-14

Maintain two distinct conclusions: this amendment's readiness disposition and Sley 2.0's overall release disposition. Readiness PASS does not imply GOLD_PASS, GA, benchmark completion, publication authorization, or ZJX support.

Before merging a repair, create an evidence-impact map identifying which previous reviews, test results, benchmark results, artifact-only checks, reproducibility evidence, SBOM/provenance records, and checksums remain valid or must be regenerated. Apply the active GOLD invalidation rules; do not carry evidence across changed behavior by assertion.

Preserve the previous candidate and its artifacts. Changed executable bytes require a newly identified candidate and the applicable qualification work. Never overwrite an immutable artifact or silently reuse its checksum. A records-only/doc-only exemption requires evidence of non-effect using the established release procedure.

If a required rerun needs unavailable authority, toolchain, host access, or spend, report the precise block. Do not substitute an optimistic readiness label for an unperformed release gate.

Independent review is required for the architecture/contract interpretation and final exact candidate/evidence delta. Reuse existing review roles and machinery instead of inventing another approval bureaucracy.

## 17. Stop conditions

STOP the affected implementation work and record the conflict if it requires changing SSMC1 meaning, SCB1 encoding, digest preimages, a frozen schema/field grammar, public protocol/error behavior, mandatory dependencies, canonical extension acceptance, validity ceilings, or repository acceptance semantics.

Also STOP for an unresolved concurrent-writer/checkout ambiguity, an unsupported target artifact being presented as supported, a new public artifact framework without evidence of necessity, a missing required independent review, or a requested promotion beyond current authority.

Stopping one blocked repair does not authorize discarding unrelated work or resetting existing release status. Preserve evidence and report the owning requirement, minimal proposed resolution, compatibility cost, and qualification cost. The operator decides whether to expand scope or defer the blocked feature. Do not silently waive a requirement.

## 18. Completion and release integration

The amendment may receive a local `READY_EXISTING` or `READY_REPAIRED` disposition only when every ZR requirement has passed with evidence and every applicable ZT row has passed. Otherwise its disposition is `BLOCKED` or `NOT_ASSESSED` as supported by the record. These are amendment-local report terms, not new SMP1 or canonical error/status values.

The closeout MUST contain the actual baseline and candidate identities, requirement matrix, discovered insertion point, existing-versus-added code summary, compatibility evidence, resource-boundary map, witness results, dependency evidence, release-impact map, reviewer result, unresolved findings, and separate current GA decision.

It MUST explicitly state that no ZJX version/edition/backend/profile was selected, no ZJX support is shipped by this amendment, and no publication authority was inferred. No unresolved mandatory readiness obligation can be disguised as an optional follow-on.

If all seams are already present, document the architecture, preserve the witness, close the amendment, and return to the existing Sley release work. Do not add production abstractions simply to make the amendment appear substantial.

## 19. Decisions deliberately deferred

The later integration specification owns the choice of ZJX release/edition; usable embedding and distribution rights; library, FFI, or bounded-process boundary; supported inner artifact kinds; outer envelope/version/IDs; content recognition; profile and dictionary rules; streaming/random-access guarantees; resource and throughput targets; deterministic outer encoding requirements; trust/signature policy; update compatibility; optional/default distribution behavior; and user-visible machine tooling.

It also owns any structure-aware transform. Such a transform can remain a transport optimization only when it reconstructs the required canonical bytes and reuses Sley's authoritative validation. Semantic rewrites, migrations, or new program identities require their own Sley authority and are not implied by this amendment.

Do not claim transport byte compression reduces an LLM's context-window token use. Context selection/representation and compressed storage/transfer are separate properties requiring separate measurements. No speed, size, adoption, security, or enterprise-boundary claim is established by scaffolding.

## 20. Audit references

Selected repository sources examined at the audit commit in section 2:

- `README.md`: machine-native lineage, no Sley source parser or canonical text.
- `ARCHITECTURE.md`: single semantic authority, dependency law, identities, durability, migration.
- `docs/spec/SCB1.md`: exact envelope, closed fields/extensions, limits, errors, epochs.
- `docs/spec/REPOSITORY_PACK_V1.md`: exact ZJX transport permission, identity, profile 0, closure, preflight, failures.
- `docs/spec/REPOSITORY_MODEL_V1.md`: root/ancestry distinction and repository ownership.
- `docs/spec/REPOSITORY_EXCHANGE_V1.md`: composition of existing contracts, exchange identity and closure.
- `crates/sley-repo/src/lib.rs`, selected ranges 1–480: byte-oriented pack APIs and preflight/persistence split.
- `crates/sley-repo/src/exchange.rs`, selected range 1–110: composition, import ownership, existing ceilings.

Supporting uploaded specifications consulted: `Sley2.0mastergoal.md` and `Sley2.0.0GOLDmastergoal.md`. Their applicable active versions must be established at implementation activation; this document does not infer current activation from an uploaded filename.

External design references: RFC 2119 section 6 on restrained normative requirements; RFC 8878 section 8 on decoder resource and malformed-input safeguards. These are supporting rationale, not adoption of Zstandard or its wire format.

**Audit limitation:** This revision corrects the identified specification defects and is suitable for repository-specific review. It is not proof that the local implementation, unpushed work, tests, final artifact, or eventual ZJX backend has passed qualification.
