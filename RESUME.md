# Resume state, 2026-09-05 (night, third push: S20-330 closed, checkpoint)

The Council review round is **complete (69/69) plus two reconciliation
re-reviews (71 verdicts: 68 FAIL, 3 PASS)**. The repository is clean at
`1e71820`. Tier 1 (`make lint`; `make quick` except the by-design S20-730
staleness that re-attests at smoke) and Tier 2 (`core`, `conformance`,
`adversarial`, `fuzz-smoke`) are green at the S20-330 commits; **the
clean `release-candidate-smoke` has NOT been run since `7041a07`**: the
operator stopped development at this checkpoint, so the smoke attest of
the S20-330 closure is the first action on resume (see below).

## Where the work is

| Thing | Where |
|---|---|
| Repository | `/home/greyforge/sley2`, branch `main` |
| Review candidate | `/home/greyforge/cache/worktrees/sley2-review-2026-09-04`, detached at `9dc78fe` (four commits behind `main` now; see below) |
| Council review queue | `/home/greyforge/machineresearch/sley-2.0/council-queue/` (durable, and gitignored there) |
| Retained verdicts | `machineresearch/sley-2.0/reviews/` plus `reviews/verdicts.json` |
| Checkpoint narrative | `machineresearch/sley-2.0/COUNCIL_REVIEW_CHECKPOINT_2026-09-04.md` |

## Reviews run against an immutable candidate now

The first dispatches read the live checkout while it was being edited. The
Greyforge Git Guard recorded `Dirty repo after openclaw` on `260-vulcan-surface`
and all three `300` logs, which is the review mode's "immutable candidate"
requirement being violated: a reviewer reading a moving tree can produce
findings that match no commit.

Reviews now run inside a detached worktree pinned at `9dc78fe`. Every request
names that path, and `patient_dispatcher.sh` changes into it. Work on `main`
no longer touches what a reviewer sees. **When the round finishes, repin the
worktree (or make a new one) before starting another round, or reviewers will
be reading an old candidate.**

The round is deliberately still reading `9dc78fe` even though three fixes have
landed on `main` since. That is the point of pinning: a reviewer's findings
stay reproducible from one commit. It also means a reply may raise something
already fixed, so check a landing finding against `main` before acting on it.

## Council reviews: round complete (69/69)

All 69 dispatched and all 69 answered (68 FAIL, 1 PASS).
`machineresearch/sley-2.0/reviews/` holds every retained log,
`verdicts.json` the counts, and `p0-worklist.json` the derived P0 list.

The round raised **110 P0 entries** across 23 packages. Three replies arrived
truncated mid-object and were requeued; **check every reply ends in `}` before
counting it**. The final `740-vulcan-surface` verdict (4 P0) landed after the
`4aa867b` retain commit and is now retained here.


## Findings: 110 of 110 P0 entries closed

Closed, each reproduced before fixing:

- **S20-260** (3 entries): checked left shift; ordered-map entry order
  (`83d571a`); cell value units (`70f4a2e`).
- **S20-300** (2 entries, found independently by two reviewers): an exchange
  import adopting an index cache it never wrote (`cfdd263`).
- **S20-620** (12 entries, 7 distinct defects, the round's largest cluster):
  the handle that was not a capability boundary (`28ec1fc`); `context_bytes`
  undercounting and the arm grading itself (`48b71e9`); the swallowable guard
  (`a0d8a6f`); the unfrozen affordance allowlist and protocol failures read as
  candidate verdicts (`20e8bf4`); the shared execution controls narrowed per
  arm (`1e5e77b`).
- **S20-400** (8 entries): SMP1 frame contract revision 9 (`19587b5`).
- **S20-320** (7 entries): context-capsule contract revision 3 (`9164ba3`).
- **S20-330** (6 entries): negotiated-session contract revision 2 (`ed7fe87`).
  Repair 2026-09-05 (`9cb1fc6`): both fixes landed with closeout
  mappings but the register-first step was skipped, so the summary
  still read open 7/6 against this list's claim. Each of the thirteen
  findings was verified against `main` (oracle PASS 24 vectors,
  negotiated arm required; both stage checkers PASS; sley-protocol
  30/30; the 320 revision cross-check verified live by drift
  injection) and the dispositions recorded; the 84 count below is now
  what the register actually reads.
- **S20-520** (7 entries): merge contract revision 4 (`ee7451d`).
- **S20-630** (7 entries): succession-accounting contract revision 3
  (`5580fe6`).
- **S20-310** (4 entries): confirmed closed on `main`, not partial. The two
  Ariadne P0s (charging schedule, applicability table) closed in `d047eaf`;
  the two Nabu P0s (input binding, charging schedule) closed in `95c90df`.
  Checker `PASS`, `p0_open_count` 0.
- **S20-740** (8 entries): finding-register contract revision 2 (`524188d`).
  First-token classification, same-lane directional supersession, negation
  stripping, per-package clearance claims, frozen obligation payload,
  per-status verdict assertion. The two restricted-query Nabu `REVISE`
  records this rule surfaced were re-reviewed by Nabu to `PASS` with no
  findings (logs retained, verdicts 71 total); the `REVISE` dispositions
  stand as history with `superseded_by` naming the new fields. Register now
  reads 204 obligations (87 `PASS`, 26 `HISTORICAL_ROUND`, 79 `PENDING`, 11
  `DEFERRED`, 1 `OTHER`), result `FINDING_REGISTER_OPEN`. Closeout
  `docs/audits/S20_740_FINDING_REGISTER_CLOSEOUT.md`.
- **S20-750** (5 entries): decision-dossier contract revision 5 (`5e4f1ae`).
  Unshadowed the license/test inventories (SBOM entry now reports the true
  19 `BLOCKED` against the source it cites), counted the property-test
  absence in the test inventory (zero with harness scan, no unit-count
  substitution), and made `derive_decision` read the entries per a section 3
  mapping written into the contract, with missing/gated inputs failing
  closed. No re-review needed: the round verdicts stand as `FAIL` history.
  Closeout `docs/audits/S20_750_DECISION_DOSSIER_CLOSEOUT.md`. The register
  picked up the closure (`decision_dossier.p0_open_count` 5 to 0) on
  rebuild.
- **S20-720** (5 entries): packaging contract revision 3 (`42e5b00` plus a
  call-site repair `935a5d5`). Remaps ordered most-general-first (the old
  order shadowed the tree rule; the shipped binary carried fourteen
  `/home-remapped` paths, confirmed with `strings`), scan needles for the
  residue and username, `--require-clean` default with `--allow-dirty`
  escape, self-describing manifest (GA/publication/blockers/cleanliness
  inside the digest), honest 20.12 verb enumeration (four covered, four
  residual; open question 1 answered yes as a `workspace.create`
  extension), and a demo-limits repair (the S20-330 `max_sessions` field
  broke every demo frame; fixed with a bridge-pinned limit set). Round
  verdicts stand as `FAIL` history. Closeout
  `docs/audits/S20_720_RELEASE_CANDIDATE_CLOSEOUT.md`.
- **S20-730** (5 entries): reproducibility contract revision 3
  (`b9bab83`). Hermetic digest/shape verification in the checker,
  attestation ancestry plus artifact-surface freshness plus toolchain
  matching, carry-forward rebuilds, and a semantic/codec-and-identity
  depth axis with the `COMPLETE` limits stated. Round verdicts stand as
  `FAIL` history. Closeout
  `docs/audits/S20_730_REPRODUCIBILITY_CLOSEOUT.md`.
- Evidence commit `8722a55`: clean smoke at `e20b9ad` (2,083,924-byte
  artifact, demo 12/12, byte-identical rebuild, 4 `/sley2` / 0
  `/home-remapped` / 0 username strings); the repro report attests the
  commit single-host; `make quick` fully green including the new gates.
- **S20-710** (5 entries): SBOM/provenance contract revision 3
  (`7de2d99` code, `ad53ee8` dispositions, `651bcc3` evidence). License
  normalization plus grammar validation (the verbatim `MIT/Apache-2.0`
  is now `MIT OR Apache-2.0`; anything unparseable is
  `SBOM_COMPONENT_INCOMPLETE`), SPDX namespace bound to the candidate
  artifact as well as the inventory, and fail-closed `--check`
  (`local_build_ahead` fires only on loaded-and-differing evidence;
  missing evidence exits 1 with 74000/74004, verified live). The old
  Tier 1 hermeticity claim is retracted: the drift gates require
  candidate evidence. Fourteen new hermetic tests (29 in the file); the
  checker pins namespace, expression validity, and counts (which caught
  a stale 119 vs 120 relationship count in the summary). Round verdicts
  stand as `FAIL` history. Closeout
  `docs/audits/S20_710_STANDARDS_SBOM_CLOSEOUT.md`. A mid-session smoke
  failed `PACKAGE_TREE_DIRTY` on pre-smoke refresh drift; corrected by
  committing dispositions, discarding derived drift, and re-running
  clean.
- **S20-420** (4 entries): JSON-bridge contract revision 6 (`db631d3`
  code, `4cee004` dispositions, `5cb36ed` evidence, with `880b81e`
  conformance-report rebuild, `7ecf875` test-inventory refresh, and
  `b03f65d` fmt fix in between). Negative zero reads as the integer
  zero on both spellings (serde parses either as negative zero,
  verified in the parser source; reader normalizes, oracle agrees);
  the hello session/request/method/flags ride through the codec's new
  `validate_header` so violations keep `PROTOCOL_FRAME_INVALID`
  (the all-zero bounds stay the bridge's own shape rule, and a hello
  protocol version now reaches codec judgment instead of being
  dropped); precedence is the declared field order, disambiguated
  from lexicographic emission; integer widths are declared per field.
  Three new native tests, matrix 31 to 36 cases, oracle agrees on all
  36. Round verdicts stand as `FAIL` history. Closeout
  `docs/audits/S20_420_JSON_BRIDGE_CLOSEOUT.md`. Repair trail: first
  smoke failed the 730 conformance-report drift (cured by committing
  the deterministic rebuild), a require-clean retry clobbered the
  candidate evidence with a field-dropping FAIL the 710 builders
  correctly refused (cured by discarding drift and re-running clean),
  second smoke failed the dossier test-inventory drift (cured by
  committing the refresh), fmt-only change re-attested clean.
- **S20-430** (4 entries): thin-CLI contract revision 3 (`981d6a1`
  code, `281d781` dispositions, `cecb84c` evidence, with `6c1a722`
  test-inventory refresh, `be157c5` clippy fix, and `c14c669` closeout
  note in between). The transport feature leaves the offer, the
  negotiation, and the wire hello: the endpoint offers
  `Server::offered_hello` unedited in both modes, so the handshake and
  session identity no longer depend on `--json` and session opens
  succeed in JSON mode (reproduced live: same hello and repo gave two
  different handshake ids before the fix). Neither literal prescription
  survived contact with the transcript binding, which `session_open`
  checks byte-exact: per-mode advertisement would fork identity by mode
  and break opens against the digest, and always-offer would claim a
  JSON capability in byte mode the endpoint never exercises; all three
  reviews' underlying demands are met instead, recorded in the
  dispositions. Every answer flushes before the next frame is read with
  a final flush at exit (flush failure is `CLI_IO_FAILURE`); a
  subprocess test drives the real binary over pipes. Exit-time
  truncation did not reproduce on this toolchain (newline-free 572-byte
  and 63KB payloads cross intact); the flush is the liveness guarantee
  and defense in depth. The JSON test runs the same session in both
  modes and asserts byte-identical answers; pins are SMP1 rev10, bridge
  rev6, the audit's transport-feature ban, and new contract-checker
  markers for sections 2, 6, and 8. Round verdicts stand as `FAIL`
  history. Closeout `docs/audits/S20_430_THIN_CLI_CLOSEOUT.md`. Repair
   trail: first smoke failed the dossier test-inventory drift on the two
   new CLI tests (cured by committing the refresh), clippy
   format-collect on the new test helper fixed and re-attested clean.
- **S20-700fuzz** (4 entries): family-lane harness fix, no production-code
  change (`81cc40b` code, `d2753dd` dispositions, `d30452f` evidence).
  The lane built each fixture's request from the outer restricted fixture,
  so E2/E3/E4 died in input validation and determinism compared two
  identical input errors; it now builds from each fixture's own parameter
  types under generous limits and asserts the first execution completes
  (reproduced live: outer request dies `InputCountMismatch`, own request
  executes; 1024-run smoke over the 769-seed corpus, all eight families
  reach execution). Seeds named families they did not select; lane
  decisions sit at fixed header offsets now (all 144 family seeds
  verified by enumeration) and the runner fails unless executed runs
  cover the corpus. The refusal pins `VM_LOWER_OPCODE_UNSUPPORTED` for
  the six single-graph families; fixing the lane exposed that
  multi-function E6/E7a programs never reach the opcode check (narrowing
  is extended-only, so restricted fails the single-graph rule first),
  documented per family. Contract revision 11 binds the 128 executions
  to the vectors, states the reachability obligation, records the
  uninterpreted `ContractSource`, splits lane/vector duty. Round verdicts
  stand as `FAIL` history. Fix record in
  `docs/audits/S20_700_VM_INPUT_PERSISTENT_SLICE.md`. Repair trail: the
  first fixed-lane smoke crashed on the new pinned refusal (E6/E7a take
  the inventory path, cured by the per-family expectation, not by
  weakening the pin); the executed-count parser missed twice (libFuzzer
  prints `Done N runs`, not the stat line, and rejects `-print_stats`),
   cured and verified at executed 1024 of 769.
- **S20-390extended** (4 entries): kind-conditioned decoder rule, no new
  error code (`f4c05db` code, `620a38a` dispositions, `61c2ca3` evidence,
  with `c3048b3` conformance-report rebuild and `20ea506` test-inventory
  refresh in between). A trusted genesis asserting semantic profile 2 built
  and imported clean on both implementations (reproduced live: the public
  builder minted it, the wire accepted it, the oracle decoded kind 1 with
  profile 2), so genesis-always-1 was a writer convention, not a wire rule;
  both decoders now accept only `[1, 1, 1]` for genesis and fail anything
  else as `TXN_FIELD_SHAPE`. The contract stops contradicting revision 2 in
  its authority boundary, restates profile 1 as judged-no-operation (phase 7
  runs on operation-free programs and judges nothing) with the joint-reading
  rule, whole-state derivation, and profile-2 binding invariant, and records
  the revision-1 prose correction; ADR-0045 corrected the same way. Accepted
  corpus bytes unchanged; rejected corpus 9 to 10 vectors with a
  provided-genesis-profile vector both sides prove. Round verdicts stand as
  `FAIL` history. Repair trail: first smoke failed the 730
  conformance-report drift (cured by committing the deterministic rebuild),
  second smoke failed the dossier test-inventory drift on the new unit test
  (cured by committing the refresh), and two count pins hid in two scripts
   each (corpus count and evidence expectation updated separately).
- **S20-510** (4 entries): forward-closure inventory plus three frozen pins
  (`c56db69` code, dispositions in the same commit, `bd9fa8d` evidence, with
  a `bfcda44` test-inventory refresh in between). Reading the fingerprint
  code before fixing paid off twice: the feared divergent-`SemanticDeltaId`
  mechanism is already impossible (the frozen fingerprint enforces
  forward-closure agreement with back-reference checks and a no-extras rule),
  and the oracle already walks forward lists, so the fix unifies the one
  divergent collector rather than inventing a rule. Live repros before the
  fix: the frozen `body-only` vector carries a body delta for function 07
  with no entity delta and 07 sits in `collateral` (contract text seeded only
  `MetadataOnly`-with-body-delta); the seed-formula lines are covered by zero
  checker markers and no hashed preimage; `stored[11:43]` is uniform across
  the corpus but pinned nowhere; a temporary test with an emptied forward
  list failed `FINGERPRINT_INVENTORY_INVALID` through the back-ref pool
  (deleted after). Fix: `owned_inventory` collects exactly the forward
  closure, counts are the fingerprint input-vector lengths, the seed rule
  takes every body-delta carrier into both seed sets with collateral defined
  as reached, both-bound, and delta-free (enshrining the 07 behavior), and
  `derivation_semantics_hash` plus `delta_schema_epoch` are pinned in
  contract, checker, oracle, summary, and a native test. Corpus bytes
  unchanged (9 pairs, 5 mutations); three native tests added. Round verdicts
  stand as `FAIL` history with P1+ still open. Repair trail: single smoke
   failure, the dossier test-inventory drift on the three new tests (cured by
   committing the refresh).
- **S20-360full** (3 entries): unreachable-guard correction plus a real
  refusal matrix (`fa2fcab` code, dispositions in the same commit, `d1e031e`
  evidence, with a `4cc745f` test-inventory refresh in between). Live repro
  before fixing: a well-formed `contract_assert` program cleared phases 1
  through 11 and was refused at phase 12 with
  `CANDIDATE_OPERATION_ANALYSIS_UNSUPPORTED`, and a well-formed
  `effect_request` cleared phase 8 (stopping at the phase 9 capability gate)
  — both falsifying "refused by the owner of its own phase" and "guard
  unreachable" in one run each. Fix: section 9 states the per-opcode truth
  (only 145 refused unconditionally; 144 validated at phase 10, 160/161/162
  at phase 8) with the guard as the live path, and notes the program-wide
  phase 7 skip; the malformed matrix test comment states its malformed-only
  scope; two new tests prove the well-formed paths (144 and 162 refused at
  phase 12; 160 and 161 pass phase 8) plus a shaped-145 unconditionality
  case; ADR-0044 decision 4 corrected with its inverted consequence. Corpus
  untouched (native tests only, no fixture bytes). Round verdicts stand as
  `FAIL` history with P1+ still open. Repair trail: single smoke failure,
  the dossier test-inventory drift on the two new tests (cured by committing
   the refresh).
- **S20-780** (3 entries, the last cluster): register-truth correction, no
  code behavior change (`17c6bf8` fix, `c7ca0f1` evidence, clean smoke first
  attempt). Live repros before fixing: the rev-1 register carried 3 entries
  with zero mentions of the reimplemented concepts the matrix names; the old
  audit predicate gates `FAIL` on a recorded audit; three non-sentinel
  identifiers sit in the adapter while the gate passes. Fix: sections 1.3
  through 1.5 add seven-field reimplementation dispositions (identity,
  typed checking, effects/authority) citing the in-repo specs and the frozen
  suites that pin them (sley-id 7, sley-ssmc 10, sley-check 75, check
  effects 16, policy capability 10, all executed live); the matrix is
  retained as succession evidence rather than retired; section 2.1 states
  exactly the single-sentinel search with non-mechanized detection named as
  a limit; the checker records the audit fact as a boolean instead of
   freezing it false. No tests added, corpus untouched. Round verdicts stood
   as `FAIL` history until the P1+ pilot below superseded them.
- **S20-780 pilot** (P1+ backlog pilot, all severities): register revisions 3
  (`d149c68`) and 4 (`57f509e`) plus three live Council re-reviews
  (`2798f1b`), status `S20_780_REGISTER_ACCEPTED`. Rev3 addressed the eleven
  P1, eleven P2, and ten P3 findings (entry split, staged-execution truth,
  reimplementation criterion, exact sentinel/dependency/executable inventory,
  lineage enforcement, audit recorded as a remaining gate). First re-review
  round: Ariadne PASS and Vulcan PASS with no new findings, Nabu FAIL with
  two residuals (lockfile stanza parsing, transcript-tree bounding); rev4
  closed both and Nabu PASS followed. New `ariadne_review`, `nabu_review`,
  and `vulcan_review` PASS obligations supersede the three FAIL rounds to
  `HISTORICAL_ROUND`; every 780 open claim reads zero. Four re-review
  transcripts recorded. Repair trail: one `finding-register:drift` (rebuilt
  the derived register after the summary change) and one stale-counter sync
   (`sync_evidence_counters.py` for obligations 204→207, open reviews
   79→76); smoke passed clean on the first attempt after each commit.
- **S20-360 package** (single Nabu round, 0 P0 / 2 P1 / 6 P2 / 3 P3):
  `6278eca` plus cascade `9670053`/`d02f2ee` and two live Nabu re-reviews
  (`d1d6530`, `43d89f6`). P1-1: judgment runs the canonical-constant check
  in lowering position. P1-2: section 3.1 normative invariant plus
  differential test, hardened after the first re-review FAIL (same-code
  agreement, closed lower-only code set, entry-gate pin, structural checker
  pins). P2-1/P3-2: ignored-fields and caller-supplies statements. P2-2/P2-5
  already corrected (section 9, ADR-0044). P2-3: excluded-opcode invariant
  with the table moved to the closeout addendum. P2-4: phase 7 analyzability
  flag (identity cascade through receipt and exchange fixtures re-emitted).
  P2-6: Ariadne Q1 decision in sections 5 and 8.2 with the closed VM_LOWER
  set. P3-1/P3-3: defense-in-depth note, landing-revision note. New
  `nabu_review` PASS obligation supersedes the round; all three 360 rounds
  are historical, every 360 claim reads zero. Two re-review transcripts
  recorded. Repair trail: reproducibility-report staleness (re-attest after
  surface changes), dossier test-inventory drift, two clippy pattern lints,
   and the fixture/evidence cascade in dependency order; smoke passed clean
   on the first attempt after each commit.
- **S20-260/270 package** (Ariadne 6 P1 / 5 P2 / 5 P3, Vulcan 2 P1 / 3 P2 /
  2 P3; Nabu already PASS, P0s already closed): contract revision 12
  (`0d8553d`, oracle bump `a6868b8`, two live re-reviews, `1f5f5ee`).
  P1-1: `lowerer_version [2, 0, 0]` separates the E6 callee-table layout in
  cache identity (all 21 prior keys rotated, old `9ddf697b` retired, the
  independent oracle pins the bump separately and re-derives all 22 keys).
  P1-2: section 2 states the real post-construction invariant (result type
  identity; `check_constant` at the input boundary only; cell handle
  register-only). P1-3/P3-1/P3-5/Vulcan P3-2: exact E1 rows (hashable, no
  contained float, bare floats admitted; order row admits `F32`/`F64`) with
  the operative `NaN == NaN` rationale. P1-4: the fixed 256 ceiling
  normatively ignores manifest field 5 (no request path delivers it;
  honoring it needs a new lowering profile). P1-5: the comparison refused
  the 256th live frame, not the 257th — fixed to `>`, pinned by a
  256-ok/257-refused boundary test plus the `call-direct-depth-ceiling`
  vector. P1-6/Vulcan P2-2: the encoding order is frozen and named as such
  (255/256 inversion pinned), and dedup plus every probe compare canonical
  key bytes. P1-7: the `-0`-to-`+0` flush is a named section 3 deviation
  with the `float_div` consequence. P1-8: the full `SLEYBC02` layout
  (header, entry body, `u64` callee count, ascending bodies) is normative in
  section 1. P2-1/P3-1: the campaign record carries revision 12, 22
  vectors, and a Tier 2 handoff. P2-2: ADR-0039 accepted at PASS time
  (header already read revision 11 with E7a). P2-3: absent-key removal and
  probe arities in the table. P2-4: `float_neg` one operand named. P2-5:
  the five-fuel derivation (1 dispatch + 1 frame + 1 op + 2 terminators).
  P3-2: the manifest wire-spelling mapping stated. P3-3: the E7 rejection
  cites `CONTRACT_TEST_PROFILE_V1` section 3.4. P3-4 verified
  already-correct (fuzz comment carries E7a). Vulcan P2-1: `max_value_units`
  bounds charged liveness, not transient peak. Vulcan P2-3: determinism
  pinned to `rust-toolchain.toml` on one triple with the required FP
  environment stated. New `ariadne_review` and `vulcan_review` PASS
  obligations supersede both FAIL rounds; all three 260/270 rounds are
  historical, every 260/270 claim reads zero. Two re-review transcripts
  recorded, both PASS first attempt. Repair trail: the register clobbered
  once by redirecting the builder's stdout over the file (restored from git,
  rebuilt with the builder's own write path), `test-inventory:drift` (two
  new tests), secret-scan drift (oracle edit), and the oracle's own
  `[1, 0, 0]` pin failing all 22 vectors until bumped; smoke passed clean
  on the first attempt after each repair.
- **S20-400 package** (Ariadne 12 P1 / 9 P2 / 4 P3, Nabu 9 P1 / 12 P2 /
  7 P3, Vulcan 8 P1 / 10 P2 / 3 P3; all P0s already closed at revision
  9): contract revision 11 (`ec90b35`, `f589779`, `fc8aa95`, `e65e3b9`,
  residual rounds `776983f`, `019f0f8`, `a4f9fb9`, `3d911f4`,
  `9b138a9`, `84bfa9c`, `7041a07`). Retryability fully enumerated (28
  `AFTER_LIMIT_CHANGE` symbols byte-identical in contract and server
  with checker-compared exact sets, `AFTER_CAPABILITY` for
  reserved-method `UNSUPPORTED`, `STALE_ROOT` named as the emitted
  symbol at 36002); response ceilings enforced on both paths
  (`max_response_bytes/entities/edges/depth`, no partial body);
  dispatch costs one unit up front plus bytes on success (renew never
  resets); identifier floor (0 sentinel, start at 1, open and genesis
  paths carry 0 unconsumed, exhaustion closes, session-less failures
  answered with 0 never echoed); negotiation floor (`session.open`
  mandatory, family as hundred-group, reserved rejected from hellos,
  epoch equality with stated server preference); version split at
  decode (below is `DOWNGRADE`, above is `VERSION_UNSUPPORTED`,
  `check_claim` agreeing); failed streams keep bit 2 everywhere with
  agreement-required reassembly; per-seam reserved reasons with
  `AFTER_CAPABILITY`; extended execute behind feature bit 4;
  transport-supplied details from a stated five-member set with
  envelope incident none; appendix A in tag order with S20-330 rows
  provisional; S20-440 frozen once in section 7; twelve 40000-40011
  registry rows; ADR-0032 accepted; the checker pins appendix coverage,
  summary/ADR/registry agreement, exact retryability, dead-token
  absence, the anchored revision, and the lane-reconciled
  register-first invariant at `IMPLEMENTED`. New `ariadne_review`,
  `nabu_review`, and `vulcan_review` PASS obligations supersede all
  three FAIL rounds; every 400 claim reads zero. Ten re-review
  transcripts recorded (Ariadne PASS round 3, Nabu PASS round 3, Vulcan
  PASS round 4, each round's residuals closed in the next commit).
  Repair trail: the SMP1 fixtures, bridge table, bridge vectors, and
  oracle method lists re-emitted (reserved tags leave the client
  hello); the S20-330 revision pin moved 10 to 11; the bridge vector
  checker compares live rows; the CLI, bridge, and release-demo suites
  follow the new wire rules (session-less identifier 0 broke the
  candidate demo's `exchange.import`, fixed in the demo driver);
  conformance-report, provenance, and secret-scan drift cured by
  committing the deterministic rebuilds in order; two clean smokes,
  the second re-attesting at the evidence commit.

- **S20-330 package** (Ariadne 7 P1 / 5 P2 / 4 P3, Nabu 7 P1 / 5 P2 /
  4 P3, Vulcan 4 P1 / 3 P2 / 3 P3; P0s already closed at revision 2):
  contract revision 3 (`34ea7f0`), residual round (`bec4468`), PASS
  record (`1e71820`). Every item was checked against `main` first; most
  were already closed by revision 2. Revision 3: the SMP1 pin follows
  SMP1 to 11 with the checker reading SMP1's and the capsule profile's
  own status lines; `renewals` is a `u16` with a checked increment; every
  frozen method tag is classified into one of five lists (the revision 2
  enumeration carried four wrong tags and called `candidate.*` mutating)
  and the checker compares the lists against the server `head_bound`
  table, `Method::tag`, the reserved set, and the SMP1 table (five drift
  shapes injected and caught); the checker binds the capsule module, the
  server's `bind_context_capsule` call, threat-matrix test existence,
  the exact symbol-numeric pair of every `SESSION_*` variant, and the
  register-first lane rule. Round 1 re-reviews: one residual each
  (Ariadne P1: the contract ordered budget before bound root while the
  server checks root first; fixed in the contract, pinned by the
  precedence test and T15 matrix; Nabu P2: exact code pairs; Vulcan P3:
  five-way wording). Round 2: three PASS. Six transcripts recorded.
  Repair trail: a `cmd | tail` pipeline masked a clippy failure once
  (absurd `>= u16::MAX` comparison, fixed with `checked_add`); `make
  conformance` was failing on clean `main` since `9b138a9` because the
  bridge vector checker's encoder lacked the SMP1 revision 11 version
  split (fixed in `d26b686`; the smoke never runs `conformance`, which
  is how it hid); T54 secret-scan drift cured by the deterministic
  rebuild each commit; the stale S20-400 row (revision 10) in
  `docs/WORK_PACKAGES.md` corrected (`f29360e`).

**No P0 entries remain open.** All 110 Council P0 findings are closed, and
the S20-780 pilot and the S20-360, S20-260/270, S20-400, and S20-330
packages closed across all severities (rounds superseded, claims zero).
The wider P1+ backlog stands at 67 open reviews in other packages;
lower-severity findings there remain tracked in the finding register.

Two patterns account for most of what has been closed, and are worth carrying
into the rest:

1. **A contract asserted an invariant nothing executed.** The map order, the
   cell units, the handle boundary and the affordance list were all like this.
   When a reviewer cites a clause, check whether anything runs it.
2. **A measurement favoured the thing it measured.** `context_bytes`,
   `invalid_candidates` and the oracle override all read low for the arm under
   test. Ask which side an error falls on.


## To resume

1. **The dispatcher is done.** `patient_dispatcher.log` ends `ALL_DISPATCHED`
   (69 round reviews plus the 2 reconciliation re-reviews); do not restart
   it. If a new round starts, repin the worktree first (see above) so
   reviewers read the new candidate. The re-review worktree
   `/home/greyforge/cache/worktrees/sley2-review-2026-09-05` (at `9c7d4ba`)
   can be removed with `git worktree remove` once no session needs it.

2. **First action on resume: attest the S20-330 closure.** Run
   `make release-candidate-smoke` on the clean tree at `1e71820`, commit
   the evidence (`evidence: re-attest the candidate clean after S20-330
   closure`), and refresh this file. Expect the usual drift repairs
   (conformance-report, provenance, secret-scan) cured by committing the
   deterministic rebuilds in order. `make conformance` now passes on
   `main` again (`d26b686`); keep running it, the smoke does not.

3. **No open P0 cluster remains, and the 780 pilot and the 360, 260/270,
   400, and 330 packages are the proven closure loop** (fix, live
   re-review via `forge handoff --timeout 2400 --thinking high <role>`,
   PASS-record, attest). Candidate next work: the next backlog package
   per the finding register (67 open reviews; by open P1 count:
   sley2_trial_runner 620, succession_accounting 630, cli 430,
   release_candidate_packaging 720, json_bridge 420,
   root_backed_query_profile 310), or a new Council round. Leftover
   noticed, not touched: `docs/spec/SLEY_CLI_V1.md` still pins SMP1
   revision 10 (S20-430's contract; carry it in that package's next
   revision).
   Triage each landing verdict into the S20-740 finding register by recording the disposition in
   `machineresearch/sley-2.0/machine-summary.json` and rebuilding, and keep
   `reviews/verdicts.json` current. Take reviewer counts from the emitted
   JSON, not the prose.

## Gates

Council model access is open. Five stand, all outside the integrator: the
narrowed schema-epoch decision, succession trials (model access plus spend
authorization), the root license text, second-host attestation, and the release
decision.

## Validation at this commit

At the S20-330 commits: `make lint`, `make core`, `make adversarial`,
`make fuzz-smoke` at the revision 3 tree; `make conformance` at
`34ea7f0` (after `d26b686`); `make quick` green except the S20-730
staleness that re-attests at smoke (every check after it run by hand and
green); `cargo test -p sley-protocol` 44/44 and clippy clean at
`bec4468`. The clean `release-candidate-smoke` last passed at `7041a07`
and is pending for this closure. The full `make v1` gate was skipped
because these revisions are subsystem handoffs, not a release boundary;
`make v2` and `make release-check` remain intentionally fail closed. One
combined Tier 2 invocation at an earlier checkpoint exited 2 once and did
not reproduce across later runs, individually or combined.

Two clippy errors exist in `fuzz/targets/root_query_engine.rs` and
`context_capsule_builder.rs` (`manual_is_multiple_of`). They are pre-existing:
`make lint` runs `cargo clippy --workspace`, which does not include the separate
`fuzz` crate, so that crate has never been linted under `-D warnings`.
