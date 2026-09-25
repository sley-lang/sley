# S20-430 Thin CLI Closeout

Status: **implemented under the draft Thin Machine-Oriented CLI v1 contract (revision 3); Council reviews pending, so the package is not complete; the Sley 2 goal remains incomplete**

Completion record (2026-09-23): contract revision 10 has PASS verdicts in all three Council lanes (scoped to 26d050e; see the machine summary lane fields and `bench/live/GATE-RECORD-20260923-CONTEXT-IMPLEMENTATION.md` section 15); status `S20_430_COMPLETE`. The open P3/P4 findings of the last rounds are recorded as `p3_open`/`p4_open` claims in the machine summary and do not block completion. The text below is retained as history.

Date: 2026-09-03

Validation tier: **Tier 1 plus protocol-focused Tier 2 handoff**

## Claim under review

`sley` is a transport endpoint and nothing else. `sley serve` moves SMP1
frames between standard input, standard output, and the deterministic
S20-410 server over one repository path, in byte form or in the S20-420
JSON form, answering each frame as it arrives or, under `--batch`, every
frame together so S20-440 cancellation can precede execution. The endpoint
offers the server's own hello (`Server::offered_hello`: version 1, the
conformance schema epoch, the limit ceilings, the dispatched
methods, cancel and stream, no adapters or effects),
derives the profile with the frozen `negotiate`, and builds only two
frames itself: its hello and one failure response through the codec. It
counts what it moved into a report, maps its own four failures to exit
statuses 2 through 5, and judges nothing: a failed answer is never a CLI
failure. `frame decode`, `frame encode`, `methods`, `hello`, and `version`
expose the bridge and the offer. The contract is `docs/spec/SLEY_CLI_V1.md`
with ADR-0035; it is a draft written and implemented while every Council
lane was unavailable, so the Ariadne, Nabu, and Vulcan reviews that freeze
it and complete the package are pending and must pass before the status
above changes.

The implementation provides:

- `sley-cli` (`crates/sley-cli/src/lib.rs`, binary `sley` in `main.rs`):
  `CliErrorCode` with the four codes 43000 through 43003 and their exit
  statuses, `CliFailure`, exact `parse`, `run` over injected streams,
  `serve` with the byte and JSON frame sources, the counting `Report`, and
  the one `failure_frame`;
- `sley-protocol`: `Server::offered_hello` and `Server::is_deferred`, so
  the offer is the server's, not the endpoint's;
- `sley-json-bridge` revision 3: method tag zero renders as the empty name
  on every frame kind (frame-level failure responses carry it) and
  `BridgeError::envelope` names a bridge rejection in the codec's failure
  record;
- the mechanical rule audit `scripts/check_cli_rules.py` in `make quick`
  (dependencies, reverse dependencies, kernel crate names, failure and
  frame literals, `encode_frame` call sites, judgment functions, method
  match arms and names).

## Evidence

- Contract draft revision 1, ADR-0035, stage checker, and rule audit at
  `d5fc242`; bridge revision 3 and the offered hello at `6e6b6bc`;
  revision 2 and the implementation at `c680894`.
- Revision 3 (contract, implementation, both checkers, closeout): the
  transport feature leaves the offer, the negotiation, and the wire
  hello; flush-per-answer plus the process-boundary test; byte-identical
  cross-mode answers.
- Endpoint tests (`cargo test -p sley-cli`, eight tests over a trusted
  genesis repository): byte-mode answers byte-identical to a direct
  `Server` over the same repository with the report's counts; JSON mode
  answering the same requests and rejecting two bad lines in place with
  `JSON_BRIDGE_SHAPE_INVALID`; batch mode cancelling a request before
  execution by both the cancel method and the cancel flag, and per-frame
  mode not; a failed negotiation answered once with
  `PROTOCOL_NO_COMMON_PROFILE` and ending the input; a prefix above the
  ceiling answered `PROTOCOL_FRAME_TOO_LARGE` without reading the body;
  thirteen usage, handshake, and input failures with their exit status,
  cause, and single stderr object, plus a short frame after the handshake
  recorded in the report; `frame decode` and `frame encode` reproducing
  the S20-420 fixture; and `methods`, `hello`, `hello --json`, and
  `version`.
- `sley-protocol` gains one test that the offered hello names exactly the
  dispatched methods (37) and that each answers something other than an
  unsupported-method failure (no deferred-method predicate exists; the
  earlier "33" and "deferred" wording was wrong).
- `scripts/check_cli_rules.py`: dependencies exactly `serde_json`,
  `sley-json-bridge`, `sley-protocol`; one frame literal; one
  `encode_frame` call; no findings.
- `cargo clippy -p sley-cli --all-targets --no-deps -- -D warnings` is
  clean; `make quick` green at the commit.
- Tier 2: see the validation record below.

## Findings closed in flight

- The bridge could not render the server's frame-level failure responses
  (method tag zero); bridge revision 3 names tag zero on every kind.
- Revision 2 of the CLI contract records the clarifications the
  implementation forced: short reads are input failures before or after
  the handshake, an over-long prefix or text line ends the input, the
  endpoint's own failure responses count as answers, unnamed endpoint
  failures are `CLI_IO_FAILURE` with a cause, `hello --json` writes the
  `Hello` object, and standard error is best effort.

## Explicitly open and deferred

- **Council reviews.** Ariadne, Nabu, and Vulcan reviews land as contract
  revisions; the campaign record lists the open questions (per-frame
  versus batch default, and whether `Server::offered_hello` belongs to
  the S20-400 contract). The `json_bridge` offering question is decided
  by revision 3: neither always nor only under `--json` — the offer
  carries no transport feature in either mode, so the handshake and
  session identity do not depend on the transport flag.
- Cancellation reaches an earlier request only in batch mode; a streaming
  transport with interleaved cancellation is out of scope by contract.
- Cross-mode answers are byte-identical for the same request frames
  (revision 3 replaces the revision 2 method/identifier/bounds/length
  comparison, whose digest rationale described the old bit-carrying
  behavior correctly but is now obsolete).

## Validation record

Tier 1 `make quick` passed at the commit. Tier 2 ran on 2026-09-03 at
`c680894` (`make core` 982 tests, `make conformance` 19 oracles,
`make adversarial` 596 tests, `make fuzz-smoke`, and both SMP1 persistent
smoke gates, all exit 0 in 40 seconds of wall time) and is recorded in
`machineresearch/sley-2.0/s20-430-thin-cli-campaign-2026-09-03.md`. The
full `make v1` gate was skipped because this is a subsystem handoff, not a
release boundary; `make v2` and `make release-check` remain intentionally
fail closed.

Revision 3 re-attestation: Tier 1 (`make quick`, `make lint`) and Tier 2
(`make core`, `make conformance`, `make adversarial`, `make fuzz-smoke`)
all exit 0 on the revision 3 tree, and `make release-candidate-smoke`
passes clean with both SBOM documents and the provenance statement
rebuilding in order and the register and dossier cascades recording the
four closed P0s.

## Independent review

Pending. Sessions and verdicts are recorded here when they land.

## Revision 6 attestation: capable runtime and review-round repairs (2026-09-10)

Revision 6 implements the section 9 version-aware surface
(`--protocol-profile v2-capable` on `hello`, `methods`, `version`,
`serve`, `frame encode`, and `frame decode`; `sley2-cli-report-v2` with
the actual `selected_protocol_version`) and repairs the three findings of
the revision 6 Council round (Ariadne FAIL P1, Nabu FAIL P2, Vulcan FAIL
3×P1) without touching version 1 defaults or legacy byte vectors:

- Post-handshake bridge rejections are stamped at the selected version
  (`failure_frame`/`write_rejection` carry the selection; only
  pre-selection rejections stay version 1) and set the failed bit (SMP1
  section 6); pinned by selection-2, selection-1, legacy-profile, and
  failed-negotiation serve tests, including a multi-frame mixed-stream
  round trip through the frame commands.
- `scripts/check_cli_rules.py` derives its tag arms and name literals
  from the frozen `Method` table (306/307, `entity.*`, and every bare
  method name audited by construction) and counts both frame-encoder
  spellings; `scripts/test_cli_rules.py` and
  `scripts/test_cli_contract.py` pin the audit and the
  current-delta-review record gate with mutations and negatives.
- The legacy-hello serve test asserts the actual selection (1), the
  report-v2 contract, a successful version 1 open, and the refused 306;
  the capable identity derivation (version-aware, filtering 306/307 on
  version 1 selections) is stated in contract section 9 with both
  identities pinned.
- Report accounting (`codes` sum to `failed_answers` for ordinary
  failures under both selections, with the stream-terminal exception
  stated in section 3), detached-flag causes, partial-stdout behavior,
  and the `Command`/report shape compat note are stated and tested.

Validation on this tree (sibling lane repair/cli-r6-council, this commit): `cargo test -p sley-cli` (25 tests: 18 pre-existing plus 7 new revision-6 tests, 4 rejection-stamping plus code-counts plus detached-cause plus partial-stdout, alongside the hardened legacy-hello test),
`scripts/check_cli_contract.py`, `scripts/check_cli_rules.py`,
`scripts/test_cli_rules.py` (10 cases, including the derivation-failure
self-test), and `scripts/test_cli_contract.py` (7 cases) all pass;
the pre-existing `check_error_symbol_registration` FAIL (unregistered
`sley-vm` `PACKAGE_*` symbols) is untouched by this delta and stays with
the registry owner. Council re-review of revision 6 stays with the S20-430
queue.
