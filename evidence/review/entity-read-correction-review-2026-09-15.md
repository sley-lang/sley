# Entity-read correction review

Date: 2026-09-15. Baseline HEAD `dd8b7a2615050344af69fd6d29893a39769db816` with the reviewed worktree corrections. Scope: the two entity-read correction summaries, their five fingerprinted source/spec files, corresponding historical Ariadne/Nabu/Vulcan findings, and relevant existing adapter/verification/codec paths. Independent static review; no source edits, builds or test reruns.

## Verdict

**Accept the scoped implementation and evidence corrections; no new correctness defect found.** Exact frame replay, arithmetic-row replay and removal of the redundant accepted-revision clone close their specific historical findings. The two requested no-action dispositions are justified below. One previously recorded root-profile checker/registration item remains **partially addressed**, so this is not unconditional closure of every entity-read review row, AT-MW-02 completion or release acceptance.

## Historical closure matrix

| Finding anchor | Reviewed evidence | Disposition |
|---|---|---|
| Vulcan entity-read 9ae09a1/246d5c4 P3: accepted response wire bytes, frame identities and lengths never compared with Rust output | `crates/sley-protocol/src/server_tests.rs:2997` iterates all 23 accepted cases, obtains session/request/method/limits from semantic input, constructs BoundedContext explicitly, uses the direct encoder actually used by Server, checks generic-encoder agreement, compares complete frozen wire/ID/length and decodes the result. Existing `accepted_corpus_vectors_match_owner_and_encoder` at `crates/sley-query/src/entity_read.rs:1904` separately derives bodies/work/count. | Closed by composition of the two evidence layers. This is direct codec/frame evidence, not issuance of a synthetic live session. |
| Ariadne246d5c4 P3: allegedly unconsumed `bound_work_exact`/`bound_work_below`; Vulcan single-test coverage overclaim | The old dispatch already consumed their relation tags. `crates/sley-query/src/entity_read.rs:2475` now pins all six exact row IDs; documentation distinguishes these six from the other three work relations. | Historical missing-coverage inference corrected; explicit guards and truthful coverage wording close the record concern. No new runtime fix is claimed. |
| Ariadne246d5c4 P4: checked-overflow test uses different operands from the declared row | `crates/sley-query/src/entity_read.rs:2564` passes exact `arith_k`, `arith_b`, `arith_l`, `arith_m` fields to production `work_bound`, whose checked operations are at `:962`. | Closed for exact arithmetic-unit replay. Does not claim those values form an admissible allocated request. |
| Ariadne246d5c4 P4: dirty refresh labelled with prior HEAD | `docs/spec/ENTITY_READ_PROFILE_V2.md:362` preserves the historical HEAD and encoder hash, explicitly records the then-uncommitted encoder repair and denies a clean-refresh interpretation. Actual current oracle SHA256 matches the disclosed value; all corpus bytes are unchanged. | Disposition accepted as honest provenance annotation. The historical refresh is not transformed into a clean checkout, and its limitation remains recorded. |
| Vulcan9ae09a1/246d5c4 P4: `Server::head` clones complete VerifiedRevision | `crates/sley-txn/src/repository.rs:331` consumes the private wrapper's sole owned revision; `crates/sley-protocol/src/server.rs:1259` uses it instead of cloning. | Redundant-copy finding closed. Full accepted-revision load/verification still occurs and remains outside query-local response/work bounds. |
| Nabu9ae09a1 P4: capture re-export versus encode wrapper, public audit views | Exact alias/facade and immutable accessor traced below. | No code action required; explicitly dispositioned as compatible API hygiene, not an implemented rename/visibility change. |
| Ariadne246d5c4 P4: `agree()` before signature kind check | Exact production VerifiedRevision precondition and defensive owner ordering traced below. | No code action required under existing trusted-input contract. Defensive off-precondition refusal remains. |
| Nabu9ae09a1 P4: root checker requires only accepted/SHA files and does not register the independent entity checker in root section 11/summary | Entity spec now names `check_entity_read_vectors.py` and its oracle, but `scripts/check_root_backed_query_profile.py:164` still requires only accepted.json and SHA256SUMS, and root section 11/summary registration remains as previously audited. | **Partial, carried residual.** Naming the checker in the entity spec improves ownership documentation but does not implement the exact root-profile registration/existence binding from the historical finding. This is not a new regression. |

The existing sequence tests and adapter projection tests remain separate prior corrections. Their status is not newly inferred from this review's acceptance of another package or lane.

## Independent assessment of no-action rationales

### Adapter/API hygiene

`crates/sley-repo/src/entity_read.rs:18` re-exports the actual query-owned capture function. Its encoding facade at `:104` directly delegates with the same owned plan, session and error types. The production caller at `crates/sley-protocol/src/server.rs:1175` captures only after body/frame preflight and budget reservation. Neither spelling introduces a second semantic implementation, new object lookup or authority constructor.

`EntityReadSelection` keeps private state and `views()` at `crates/sley-query/src/entity_read.rs:296` returns an immutable borrowed slice. It cannot alter selection bindings or obtain session reservation authority. A redundant capture facade or removal of existing public methods would change API surface without correcting a demonstrated invariant. Preserve them and record the asymmetry/accessor as accepted compatibility choices. This disposition does not claim arbitrary pure-owner callers are authenticated or budget-admitted.

### Defensive `agree()` ordering

`resolve_target` at `crates/sley-query/src/entity_read.rs:660` performs `agree()` before `select_signature` checks Function kind at `:548`. The check verifies target/binding ObjectId and epoch, not semantic kind. The actual repository projection at `crates/sley-repo/src/entity_read.rs:42` derives identity/kind/body/bytes from one imported object in one VerifiedRevision. `load_verified_revision` (`crates/sley-txn/src/repository.rs:1989`) and `validate_inventory` (`:2727`) establish exact imported object, entity, object ID and epoch agreement against root bindings before constructing that private verified type.

A legitimate non-Function target therefore reaches ClassNotApplicable; an inconsistent hand-built pure-owner view receives InternalInvariant earlier. The existing contract identifies inconsistent verified bodies as invariants, and the historical reviewer already identified the ordering as unreachable from a valid VerifiedRevision. Keeping the defensive check preserves this behavior and introduces no wire precedence change. A future expanded contract promising precedence for inconsistent public views would need separate specification/review.

## Consuming accessor regression assessment

`AcceptedHead` contains exactly `revision: VerifiedRevision`; the added method moves that field. Existing borrowed access remains available. The loading, receipt/ancestry/object validation, maintenance lifetime, head instrumentation and owner-error mapping preceding the map are unchanged. No new constructor or mutable access is introduced. Thus the change removes one deep copy while preserving the trust boundary and caller-visible data.

## Validation inspected

Both `entity-read-correction-files.json` and `entity-read-residual-files.json` match all five current source/spec file hashes. The four files under `conformance/entity-read/v2` are byte-identical to HEAD. Current oracle SHA256 is `63e82fc8ee66a6b6d10ca18e978ee4cbc1aebef4998f24990b616d93470d9a35`, matching the preserved manifest and new annotation.

Read the recorded logs: query entity-read 34 passed; protocol 87 passed with 3 existing ignored; transaction accepted-head selection 14 passed; both relevant clippy logs completed without warnings/errors. The correction summary also records an additional two-test boundary rerun and restored benign test assertion controls. These are supplied execution records, not tests independently rerun by this reviewer. No packaged build or release requalification was requested or performed.

## Remaining scope

To close the carried Nabu registration P4, add explicit root-profile bindings for the independent checker and all four corpus files, or provide an explicit scoped acceptance of that residual. Neither whole entity-read lane tokens nor global readiness should be upgraded merely from this report. AT-MW-02 consumer demonstrations and inherited full-root loading costs remain separate limits.


## Registration delta re-review — 2026-09-15

**The carried Nabu root-profile registration P4 is now closed.** Static review of the follow-up confirms `scripts/check_root_backed_query_profile.py:23` names the actual entity checker and oracle; its required-path list at `:166` includes both and all four corpus files (accepted, rejected, inputs, SHA256SUMS). Its comparison loop at `:219` binds both exact relative paths to summary keys and requires those paths in the root contract. Root section 11 at `docs/spec/ROOT_BACKED_QUERY_PROFILE_V1.md:533` and `machine-summary.json:906` now carry those registrations. This corrects the precise original omission; it does not turn registration checks into independent execution of the oracle. The existing Make invocation remains that execution owner.

### Final disposition of the three historical entity-read rows

- **Ariadne246d5c4:** both P3s are resolved (corrected sequence mapping with the two exact missing live tests present; mistaken bound-work coverage inference corrected and explicit coverage pinned). P4 exact overflow operands are repaired; dirty-refresh provenance is explicitly retained/dispositioned; agree-before-kind is accepted as a defensive invariant under the verified-input precondition. No actionable residual from that historical row remains.
- **Nabu9ae09a1:** the prior adapter projection test, completion-gate entity fields, WORK_PACKAGES composition and ERROR_CODES reuse repairs remain in current source. The refreshed encoder hash matches current bytes; the checker/corpus registration residue is now repaired; API re-export/wrapper and immutable views hygiene is explicitly dispositioned without a compatibility-breaking cosmetic edit. No actionable residual from that historical row remains.
- **Vulcan246d5c4:** sequence-map obligations and tests are present, the 23 accepted frame bytes/IDs/lengths now have production-encoder comparisons, work coverage wording is corrected, the full-revision copy is removed, and `scripts/build_finding_register.py:111`/`:153` retains subject-core-aware supersession. No actionable residual from that historical row remains.

Current scoped conclusion: **all findings on these three historical entity-read rows are repaired, corrected as erroneous findings, or explicitly dispositioned with the rationale above.** Any summary normalization must preserve those distinctions, original transcript identities and original values; provenance and hygiene dispositions must not be represented as implementation changes. This is independent item-level closure, not three new named-lane reviews. AT-MW-02 consumer demonstration/completion, the separate root-query review subjects and global release readiness remain outside this conclusion.

No source edits or additional tests/builds were performed for this delta. Parent-reported stage PASS and missing-summary-key negative control were not represented as independently executed checks.
