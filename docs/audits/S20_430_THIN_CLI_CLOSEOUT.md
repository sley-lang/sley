# S20-430 Thin CLI Closeout

Status: **implemented under the draft Thin Machine-Oriented CLI v1 contract (revision 3); Council reviews pending, so the package is not complete; the Sley 2 goal remains incomplete**

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

## Independent review

Pending. Sessions and verdicts are recorded here when they land.
