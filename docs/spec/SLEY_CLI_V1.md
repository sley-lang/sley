# Thin Machine-Oriented CLI v1

Status: S20-430 contract draft, revision 10 (2026-09-23); Council review
pending (Ariadne contract review, Nabu architecture review, Vulcan surface
review). Revision 2 records the clarifications found while implementing
revision 1 (section 8); revision 3 removes the transport feature from the
offer (section 8); revision 4 re-pins SMP1 revision 11 and bridge revision 7
(no behavior change; `scripts/check_cli_contract.py` asserts both pins
against the composed status lines); revision 5 re-pins SMP1 revision 12
and bridge revision 8 and declares the prospective version-aware surface
(section 9); revision 6 implements the section 9 surface (profile flag,
expected-version frame rule, capable metadata/report, version-aware serve)
and synchronizes the CLI revision pins; revision 7 re-pins bridge
revision 9; revision 8 re-pins bridge revision 10, states the end-of-input
rule for every bridge ceiling (section 8), and derives the test fixtures'
revision from the checker. Command defaults, version/report v1 shapes, and
legacy behavior are unchanged. Revision 9 (2026-09-23) re-pinned SMP1
revision 14 and bridge revision 11; it replaced an in-place revision 8 note
that had briefly pinned SMP1 revision 13 without a CLI revision (withdrawn
under the SMP1 revision 13 Ariadne ruling). Revision 10 (2026-09-23,
section 8) answers the revision 9 round (REVISE x3 on 2b0f1c9): it admits
the shipped version 3 capable surface (section 9) and the private
native-test worker entry with its `sley-test-runner` edge (section 10),
states the worker's argv and exit statuses, and re-pins SMP1 revision 15
and bridge revision 12. Its new-delta review is pending. An errata note
to revision 10 (section 8, 2026-09-25, Sley 2.0.1) records two
implementation fixes: a command-line word that is not valid Unicode is
`CLI_USAGE_INVALID`, and the JSON text ceiling is judged before UTF-8
decoding. It keeps revision 10 and every pin. The
implementation is `crates/sley-cli`; implementation state is tracked in
the machine summary.

The CLI is a transport endpoint and nothing else, with one bounded
exception: the private native-test worker entry of section 10, which is
not a user command. It moves SMP1 frames
between standard input, standard output, and the deterministic S20-410
server over one repository path, in either the canonical byte form or the
S20-420 JSON form, and it writes a machine-readable invocation report. It
owns no semantics: every judgment about a frame comes from the server
(`docs/spec/SMP1.md` revision 15, S20-440 batch admission, S20-330
sessions) and every representation from the frozen codec or the bridge
(`docs/spec/SMP1_JSON_BRIDGE_V1.md` revision 12). The master goal requires a thin
machine-oriented wrapper that contains no private validation rules and that
the semantic kernel never imports (master goal sections 14.2, 14.3, 22.6).

## 1. Commands

```text
sley serve --repository <path> [--json] [--batch] [--report <path>] [--protocol-profile v2-capable|v3-capable]
sley frame decode [--protocol-profile v2-capable|v3-capable --expected-version 1|2|3]
                                        # stdin: frames as bytes; stdout: one Frame object per line
sley frame encode [--protocol-profile v2-capable|v3-capable --expected-version 1|2|3]
                                        # stdin: one Frame object per line; stdout: frames as bytes
sley methods [--protocol-profile v2-capable|v3-capable]
                                        # stdout: the generated method table (the version 2 or
                                        # version 3 table under the profile)
sley hello [--json] [--protocol-profile v2-capable|v3-capable]
                                        # stdout: the hello this endpoint offers, as a hello frame
                                        # (under --json: the same Hello object, rendered as JSON)
sley version [--protocol-profile v2-capable|v3-capable]
                                        # stdout: {"cli":"1","contract":"sley2-cli-v1","protocol_version":1}
                                        # (under v2-capable: {"cli":"1","contract":"sley2-cli-v2",
                                        #  "protocol_profile":"v2-capable","protocol_versions":[1,2]};
                                        #  under v3-capable: {"cli":"1","contract":"sley2-cli-v3",
                                        #  "protocol_profile":"v3-capable","protocol_versions":[1,2,3]})
```

Arguments are exact: an unknown command, a repeated or unknown option, or
a missing value is `CLI_USAGE_INVALID`. There are no abbreviations, no
environment variables, no configuration files, and no prose output on any
stream. `--protocol-profile` takes exactly `v2-capable` or `v3-capable`;
any other value is `CLI_USAGE_INVALID`. On the frame commands the profile
requires `--expected-version` and `--expected-version` requires the
profile: `v2-capable` admits `--expected-version 1|2` and `v3-capable`
admits `--expected-version 1|2|3`, and a version outside the profile's
offer (3 under `v2-capable`) is `CLI_USAGE_INVALID` with cause
`--expected-version`. `--expected-version` on any other command is
`CLI_USAGE_INVALID`. The private `__native-test-worker` entry (section 10)
is not a user command and is not listed above.

## 2. `serve`

The endpoint offers the hello of the deterministic server
(`Server::offered_hello`), unedited: protocol version 1, the frozen
conformance schema epoch, the limit ceilings, every method the server
dispatches (reserved methods are not offered), the cancel and stream
features, and no adapters or effects. The offer never carries a transport feature:
the wire form (byte frames or JSON lines) is a transport choice
the endpoint makes, not a negotiated capability, so `--json` must not move
the negotiated profile, the handshake identity, or the session identity,
and the same client hello over the same repository negotiates the same
session in both modes. (A per-mode feature bit would enter the
transcript-bound handshake digest and fork session identity by mode while
breaking session opens against the digest; offering it always would claim
a JSON capability in byte mode the endpoint does not exercise. Hence
neither.) The first frame read must be the client hello
(SMP1 section 2); anything else, or no frame at all, is
`CLI_HANDSHAKE_REQUIRED`. The endpoint derives the selected profile with
the frozen `negotiate`, writes its own hello frame, and answers every
later frame through the server under that profile. If negotiation fails
the endpoint writes one response frame carrying the codec's failure
(`PROTOCOL_NO_COMMON_PROFILE` or the validation code, request identifier
zero, no session) and stops reading.

Frames are read in arrival order. Without `--batch` each frame is answered
as it is read (`Server::answer`), so a cancellation only reaches a request
that has not yet been read. With `--batch` every frame up to end of input is
read first and answered together (`Server::answer_batch`, S20-440 appendix
B), so cancellations within the input take effect before execution. For
every answer the endpoint writes the event frames, then the response frame,
in the server's order. It never reorders, drops, merges, or rewrites a
frame.

Byte mode reads the eight-byte length prefix, checks it with the codec's
`frame_length` under the ceiling in force (the absolute ceiling before the
handshake, the negotiated `max_frame_bytes` after it), then reads exactly
that many bytes; the bytes are handed to the server unchanged. Every
answer is flushed before the next frame is read, and the streams are
flushed once more at exit, so a pipe consumer never waits on a response
sitting in the endpoint's buffer and `process::exit` never drops buffered
bytes; a flush failure is `CLI_IO_FAILURE`. A prefix
above the ceiling is answered with the codec's `PROTOCOL_FRAME_TOO_LARGE`
without reading the body. A short read inside a frame is
`CLI_INPUT_INVALID`. JSON mode reads one `Frame` object per line
(terminated by `\n`), converts it with `frame_from_json`, and writes one
`Frame` object per line with `frame_to_json`; a line the bridge rejects is
answered with a response frame carrying the bridge's own code and symbol
(request identifier zero, no session) and reading continues. Before a
selection exists the rejection travels at frame version 1; past the
handshake it is stamped at the selected version (1, 2, or 3) and rendered
under it, so a capable stream never mixes versions, and the response frame
sets the failed bit (SMP1 section 6). End of input
ends the invocation; the endpoint never waits for a close.

Under `--protocol-profile v2-capable` the endpoint offers
`Server::offered_hello_versioned` (protocol versions 1 and 2 with the
version 2 method table), derives the selection with the frozen
version-aware negotiation, and answers through `Server::new_versioned`,
so decoding, dispatch, and response framing all follow the selection;
a legacy client hello still negotiates version 1 and the version 2
methods stay undispatched on that selection. A negotiation failure is
answered exactly as above, and the rejection frame travels at frame
version 1: without a selection no version is negotiated, and a
version 1 peer reads it.

Under `--protocol-profile v3-capable` the endpoint offers
`Server::offered_hello_v3` (protocol versions 1, 2, and 3; the version 3
method table, the union of the SMP1 version 1 and version 2 tables plus
the rows of `docs/spec/NATIVE_TEST_ADMISSION_V1.md` appendix D, with the
still-reserved rows unoffered; the cancel, stream, and native-tests
features), derives the selection with the same version-aware negotiation,
and answers through `Server::new_versioned`. The profile never forces
selection 3: a version 2 client hello selects 2 and a legacy one selects
1, each exactly as under `v2-capable` over the same hellos.

## 3. Report

With `--report <path>` the endpoint writes, at exit, one JSON object
rendered with the bridge's declared encodings (lexicographic fields, no
insignificant whitespace):

```text
Report {
  "contract": "sley2-cli-report-v1",
  "command": "serve",
  "mode": "bytes" | "json",
  "batch": bool,
  "handshake_id": hex[64] | null,
  "frames_read": integer, "frames_written": integer,
  "events_written": integer,
  "answers": integer, "failed_answers": integer,
  "codes": { "<numeric>": integer, ... },   // failure codes copied from failure bodies
  "cli_failure": null | { "code": integer, "symbol": string, "cause": string | null },
  "exit_code": integer
}
```

The report counts and copies; it interprets nothing. `codes` is keyed by
the numeric code of a failed answer's `ProtocolFailure` body whenever the
terminal body decodes as one under the selection, so for ordinary failures
the code counts sum to `failed_answers` under both selections; a failed
answer whose terminal body does not decode as a failure (a failed stream's
empty terminal frame, or an undecodable answer) still increments
`failed_answers` without contributing a code. `cause`
carries the underlying codec or bridge symbol, or the bare offending word
for usage and handshake-shape failures, when a CLI failure wraps one.

Under `--protocol-profile v2-capable` the report is the additive contract
`sley2-cli-report-v2`: the section 3 counters and errors unchanged, plus
`protocol_profile` (`v2-capable`) and `selected_protocol_version`
(`null` before a successful negotiation, otherwise the actual 1 or 2):

```text
Report {
  "contract": "sley2-cli-report-v2",
  "command": "serve",
  "mode": "bytes" | "json",
  "batch": bool,
  "handshake_id": hex[64] | null,
  "frames_read": integer, "frames_written": integer,
  "events_written": integer,
  "answers": integer, "failed_answers": integer,
  "codes": { "<numeric>": integer, ... },
  "cli_failure": null | { "code": integer, "symbol": string, "cause": string | null },
  "exit_code": integer,
  "protocol_profile": "v2-capable",
  "selected_protocol_version": 1 | 2 | null
}
```

Under `--protocol-profile v3-capable` the report is `sley2-cli-report-v3`:
the same fields as `sley2-cli-report-v2` with `protocol_profile`
`v3-capable` and `selected_protocol_version` `1 | 2 | 3 | null`.

## 4. Exit status and stable failures

| Numeric | Symbolic code | Exit status |
|---:|---|---:|
| 43000 | `CLI_USAGE_INVALID` | 2 |
| 43001 | `CLI_INPUT_INVALID` | 3 |
| 43002 | `CLI_IO_FAILURE` | 4 |
| 43003 | `CLI_HANDSHAKE_REQUIRED` | 5 |

Exit status 0 means the endpoint ran to end of input and every frame was
answered; a failed answer is not a CLI failure and never changes the exit
status. A CLI failure is written to standard error as one JSON object
(`{"code": integer, "symbol": string, "cause": string | null}`) and to the
report when one was requested; nothing else is ever written to standard
error. The private worker entry of section 10 is the one exception: its
statuses (1, 6, 7, 8) are the worker's own, disjoint from this table, and
it writes nothing to standard error.

## 5. Rules audited mechanically

`scripts/check_cli_rules.py` fails closed when any of these drifts:

- `crates/sley-cli` depends only on `sley-protocol`, `sley-json-bridge`,
  `serde_json`, and, for the section 10 worker entry only,
  `sley-test-runner` (which links `sley-vm`, `sley-scb1`, `sley-tests`, and
  `sley-id` into the binary); development dependencies may add `sley-repo`
  with `test-support`, `sley-id`, and `sley-scb1` for fixtures, and
  `sley-test-runner` and `sley-vm` for the worker test;
- the production source names `sley_test_runner` exactly once, as the
  worker call `sley_test_runner::worker::run_input_path(`, and names the
  `"__native-test-worker"` command word exactly once;
- no other workspace crate depends on `sley-cli`;
- the CLI source names no kernel crate (`sley_ssmc`, `sley_check`,
  `sley_query`, `sley_mutate`, `sley_txn`, `sley_repo`, `sley_policy`,
  `sley_vm`, `sley_state_root`, `sley_store`, `sley_adapter`,
  `sley_schema`, `sley_scb1`) outside its tests;
- the CLI source constructs no `ProtocolFailure {` literal, no owner
  record, and no `ProtocolFrame {` literal other than the endpoint's own
  failure frame, and calls the frame encoder (`encode_frame` or
  `encode_frame_for_version`) only there;
- the CLI source contains no `fn validate`, `fn judge`, `fn check_`, no
  method-name or method-tag match arms, and no text output that is not a
  frame, a report, the version object, or the section 10 worker's refusal
  words; the tag arms are derived from
  every frozen `Method` tag (306 and 307 included) and the name literals
  from every dotted family (including `entity.`) and every bare method
  name, so no operation escapes the audit;
- the CLI source never names `FEATURE_JSON_BRIDGE`: the offer carries no
  transport feature (section 2).
- `scripts/test_cli_rules.py` pins the audit itself with 306/307 match-arm,
  `entity.*` and bare-name literal, and second-encode-call mutations;
  `scripts/test_cli_contract.py` pins the current-delta-review record gate
  with stale-revision, missing-record, and frozen-with-pending negatives,
  and `scripts/test_cli_rules.py` pins the worker bound (another
  `sley_test_runner` path, an import, a second worker call or command
  word, and a removed worker call are each refused).

## 6. Required evidence

- Native tests over a trusted genesis repository: the handshake and a
  session opened through standard input in byte mode and in JSON mode with
  byte-identical answers for the same request frames to a direct `Server`
  over the same repository (only the session-open bodies differ, each
  carrying its own freshly minted identity); the mode-independent
  handshake identity against the direct server's digest; the batch mode
  cancelling a request before execution; the negotiation
  failure response; every CLI failure with its exit status and stderr
  object; the report's counts against the frames written; `frame decode`
  and `frame encode` reproducing the S20-420 fixture; `methods`, `hello`,
  and `version` outputs.
- Process-boundary evidence: at least one test driving the real `sley`
  binary over pipes, proving byte-mode answers cross the process boundary
  unfragmented and exit statuses propagate; the flush-per-answer
  obligation of section 2 holds on every write path.
- `scripts/check_cli_rules.py` in `make quick`.
- Capable-profile tests over the same trusted repository: the `[1,2]`
  offer carrying exactly the version 2 methods, the capable metadata and
  report contracts, a version-aware serve reporting the actual selected
  version (2 for a version-aware client hello, 1 for a legacy one, with a
  successful version 1 open and a refused 306), post-handshake rejections
  stamped at the selection with multi-frame mixed-stream evidence, the
  code counts summing to `failed_answers` under both selections, and
  the expected-version frame rule with its rejections (hello under
  expected 2, mixed versions, missing or detached flags with their
  section 9 causes, partial stdout before a frame-command failure).
- Version 3 capable-profile tests: the `[1,2,3]` offer with the version 3
  table and the native-tests feature, the `sley2-cli-v3` metadata, the
  version 3 table verbatim, a version-aware serve reporting the actual
  selection (3, 2, or 1) in `sley2-cli-report-v3`, and the expected-version
  rule under `v3-capable`.
- Worker entry evidence: the transient unit argv rendered by
  `sley_test_runner::unit::render_transient_unit`, run against the real
  `sley` binary, reaching the unwired refusal (status 6), a malformed
  input (status 1), and an absent binding (status 7), with any other argv
  shape a CLI usage failure (status 2).
- Tier 1 plus Tier 2 validation, and the Ariadne, Nabu, and Vulcan
  reviews with every report-grade finding closed.

## 7. Explicit exclusions

This contract does not claim: network transport, authentication, or
multi-tenant isolation; a daemon or socket server; interactive use, prompts,
colour, or prose; command-count parity with any legacy CLI (master goal
section 15.2); the benchmark harness (`sley-bench`); runtime, packaging,
release, or GA.

## 8. Revision 2 clarifications

- A short read (fewer than eight prefix bytes, or fewer body bytes than
  the prefix names) is `CLI_INPUT_INVALID` whether it happens before or
  after the handshake; only a complete frame that is not a client hello,
  or no frame at all, is `CLI_HANDSHAKE_REQUIRED`.
- A prefix above the ceiling is handed to the server as it stands, so the
  answer carries the codec's `PROTOCOL_FRAME_TOO_LARGE`; because the body
  was not read the stream cannot be resynchronised, and the invocation
  treats it as end of input. A JSON line the bridge refuses with
  `JSON_BRIDGE_RESOURCE_LIMIT` for any of its ceilings (text bytes, nesting
  depth, or value positions; bridge contract section 3) is answered with
  that code and likewise ends the input: the line was read in full, but a
  producer that exceeds a ceiling is not resynchronised.
- `answers` and `failed_answers` count every response frame written,
  including the endpoint's own failure responses (negotiation failure and
  bridge rejections), and `codes` counts their codes; `frames_read` counts
  every frame or line taken from the input, rejected lines included.
- An endpoint failure the contract does not name (the offered hello or a
  frame the codec cannot encode, both `PROTOCOL_INTERNAL_INVARIANT`, or a
  server frame the bridge cannot render) is `CLI_IO_FAILURE` with the
  underlying symbol as its cause.
- `hello --json` writes the `Hello` object the endpoint offers, rendered
  as JSON; both renderings carry the same offer, which has no transport
  feature in either mode.
- Standard error is best effort: the exit status carries the code even
  when the failure object cannot be written.

### Revision 3 (2026-09-05)

- The `--json` transport feature is gone from the offer, the negotiation,
  and the wire hello: the endpoint offers `Server::offered_hello`
  unedited in both modes, so the handshake and session identity no longer
  depend on the transport flag and session opens succeed in JSON mode.
  The revision 2 closeout's digest rationale described the old behavior
  correctly (the bit entered the transcript-bound handshake); the
  weakened cross-mode comparison it justified is replaced by
  byte-identical answers for the same request frames.
- Every answer is flushed before the next frame is read (section 2), and
  the process boundary is covered by a test driving the real binary.
- The revision pins are SMP1 revision 11 and bridge revision 7; the
  `version` example shows the emitted lexicographic field order; the
  "deferred methods" wording is dropped (`is_deferred` exists nowhere).

### Revision 5 (2026-09-08)

- The revision pins are SMP1 revision 12 and bridge revision 8; section 9
  declares the prospective version-aware surface. Command defaults,
  version/report shapes, and legacy behavior are unchanged; capable
  runtime is phase 3.

### Revision 6 (2026-09-09)

- Section 9 is implemented: `--protocol-profile v2-capable` on `hello`,
  `methods`, `version`, `serve`, `frame encode`, and `frame decode`;
  `--expected-version 1|2` on the frame commands with exact wire
  semantics; the additive `sley2-cli-v2` metadata and
  `sley2-cli-report-v2` report; and the version-aware serve path
  (`Server::offered_hello_versioned`, version-aware negotiation,
  `Server::new_versioned`). Command defaults, version/report v1 shapes,
  and legacy behavior are unchanged, and every legacy byte vector still
  passes unmodified.
- The revision pins stay SMP1 revision 12 and bridge revision 8: no
  normative protocol or bridge change was needed, so SMP1 keeps revision
  12 with a composition-only CLI pin move, and the bridge keeps revision
  8 (the version 2 table artifact it already specifies).

### Revision 7 (2026-09-14)

- Re-pins bridge revision 9 (`scripts/check_cli_contract.py` asserts both
  pins against the composed status lines). The CLI code is unchanged, but
  the bridge's revision-9 element ceiling reaches the CLI through
  `frame_from_json`: a fully read JSON line with 1,048,576 or more value
  positions is answered `JSON_BRIDGE_RESOURCE_LIMIT` and ends the input,
  exactly as the text ceiling already did. The revision-7 note that this
  "changes nothing the CLI parses" was an overstatement; revision 8
  corrects it.

### Revision 8 (2026-09-14)

- Re-pins bridge revision 10 (no behavior change; the bridge revision
  corrects its own hello-version wording and boundary statement).
- Section 8 states the end-of-input rule for every bridge ceiling, not
  only the text ceiling, and `crates/sley-cli/tests/cli.rs` drives the
  element-ceiling stream end in JSON mode.
- `scripts/test_cli_contract.py` and
  `scripts/test_current_contract_review.py` derive the record revision
  from the checker instead of a literal, and every `scripts/test_*.py`
  suite runs under `make quick`, so a revision move can no longer leave
  the cited gate tests red unobserved (current-delta review round on
  revision 7, Ariadne P2 and Nabu P2).
- ADR-0035 and `docs/WORK_PACKAGES.md` carry the revision-7 and
  revision-8 records.
- The revision pins are SMP1 revision 12 and bridge revision 10.

### Revision 9 (2026-09-23)

- Re-pins SMP1 revision 14 and bridge revision 11. SMP1 revision 14
  defines the `workspace.open` (201) response under version 2 and every
  later selection whose method table includes version 2's row 201 as
  `open_summary`, and refuses a non-empty 201 body under every version.
  The CLI carries 201 bodies as opaque bytes, so no CLI clause changed,
  but one observable changed through the composed server: a legacy
  (version 1) serve now answers a non-empty 201 body with a failed
  `PROTOCOL_PAYLOAD_INVALID` response (counted in `failed_answers` and
  `codes`), which the revision 8 composition answered with success.
- This replaces an in-place revision 8 note (2026-09-23) that pinned SMP1
  revision 13 without a CLI revision; that note was withdrawn under the
  SMP1 revision 13 Ariadne ruling that consumer re-pins take their own
  revision.

### Revision 10 (2026-09-23)

- Answers the revision 9 round on 2b0f1c9 (REVISE x3). The shipped
  version 3 capable surface (present since 2026-09-16) is admitted
  exactly as implemented: `--protocol-profile v3-capable`, the `[1,2,3]`
  offer, `--expected-version 3`, the version 3 table, `sley2-cli-v3`
  metadata, `sley2-cli-report-v3`, and selected version 3 (sections 1, 2,
  3, 9).
- Admits the private native-test worker entry and its `sley-test-runner`
  edge as a bounded exception (sections 4, 5, 10). The implementation
  changed to one argv contract: the entry now takes the transient unit's
  input path operand, and the worker's exit statuses moved from 1/2/3 to
  1/6/7/8 so none collides with section 4.
- ADR-0035 carries the revision 9 and revision 10 records.
- The revision pins are SMP1 revision 15 and bridge revision 12.

### Errata to revision 10 (2026-09-25, Sley 2.0.1)

These notes record two fixes in `crates/sley-cli`. Revision 10 and its
pins are unchanged.

- A command-line word that is not valid Unicode is `CLI_USAGE_INVALID`
  (43000, exit status 2). The cause is the word's lossy decoding, with
  each invalid byte sequence replaced by U+FFFD. The rule covers every
  word, including the path values of `--repository` and `--report` and
  the operand of the section 10 worker entry, and it is decided before
  any other argument check. The 2.0.0 binary panicked on such a word and
  exited with status 101, outside the section 4 table. Pinned by
  `an_argument_that_is_not_unicode_is_a_usage_failure_not_a_panic` in
  `crates/sley-cli/tests/cli.rs`.
- In JSON mode the bridge's text ceiling is judged on the bytes of a line
  before UTF-8 decoding. A line longer than the ceiling is answered
  `JSON_BRIDGE_RESOURCE_LIMIT` and ends the input, as the revision 2
  clarification above requires, even when the bytes read are not valid
  UTF-8. A line within the ceiling that is not valid UTF-8 is still
  `JSON_BRIDGE_SHAPE_INVALID`, and serving continues. The 2.0.0 binary
  decoded first: it answered an oversize non-UTF-8 line
  `JSON_BRIDGE_SHAPE_INVALID` and kept reading from the middle of the
  same line. Pinned by
  `a_json_line_above_the_text_ceiling_that_is_not_utf8_is_refused_and_ends_the_input`.

## 9. Version-aware surface (phase 3, implemented in revision 6)

The capable endpoint adopts `--protocol-profile v2-capable` for `hello`,
`methods`, `version`, `serve`, `frame encode`, and `frame decode`. The
profile enables the `[1,2]` offer; it never forces selection 2. The
capable server re-derives the handshake identity with version-aware
negotiation: under a version 1 selection the offered version-2 tags filter
out of the selection, so a client offering 306 or 307 that selects version
1 holds a different identity than the legacy derivation over the same
hellos, while a legacy client agrees with it. The
standalone frame commands (`frame encode`, `frame decode`) additionally
require `--expected-version 1|2` with exact wire semantics: Hello uses
expected 1, and every post-Hello frame uses the actual selected version
(1 or 2). Expected 2 with Hello 1 is rejected; stateless frame commands
never silently transition versions or accept mixed streams. A version
mismatch is `CLI_INPUT_INVALID` with cause VERSION_MISMATCH (a cause
word, not a registered symbol). Duplicate,
missing, or unsupported flags, `--expected-version` without the profile,
and `--expected-version` on other commands are rejected under the
existing section 4 categories. A detached `--expected-version` without the
profile fails `CLI_USAGE_INVALID` with cause `--expected-version`, and a
detached `--protocol-profile v2-capable` without `--expected-version` on a
frame command fails `CLI_USAGE_INVALID` with cause `--protocol-profile`.
There is no `--protocol-version` alias:
the generator's exact-table selector
(`scripts/generate_smp1_json_bridge_table.py --protocol-version`) is a
separate internal tool. The frame commands stream converted lines as they
read them: a failure after partial output leaves the converted prefix on
standard output and reports the failure on standard error with the
section 4 exit status.

Capable metadata is the additive contract `sley2-cli-v2` carrying
`protocol_profile` (`v2-capable`) and the offered `protocol_versions`
`[1,2]`, preserving the independent `sley2-cli-v1` contract. No fictitious
selected `protocol_version` is emitted. The capable report is
`sley2-cli-report-v2`, retaining the section 3 counters and errors and
adding `protocol_profile` (`v2-capable`) and `selected_protocol_version`
(`null` before successful negotiation, otherwise the actual 1 or 2).
Default metadata and reports retain the exact version 1 contracts and
bytes. The profile-aware entrypoint keeps existing `ServeOptions` source
compatibility with `serve` as the legacy wrapper; the `Command` variants
additionally carry the profile (the frame commands carry the expected
version too), and the report gains `protocol_profile` and
`selected_protocol_version` (section 3).

This surface is implemented in revision 6: the endpoint, crate, rule
audit, and vectors above cover the capable path, and prose presence here
is now backed by the capable-runtime markers in
`scripts/check_cli_contract.py`. The version 1 default stays frozen.

The version 3 capable profile (`--protocol-profile v3-capable`, admitted
in revision 10) is the same surface one version wider. It is accepted by
`hello`, `methods`, `version`, `serve`, `frame encode`, and `frame decode`;
it offers `[1,2,3]` and never forces selection 3; the frame commands take
`--expected-version 1|2|3` under the same stateless rule (Hello under
expected 1, every other frame at exactly the expected version); the
metadata is the additive contract `sley2-cli-v3` with `protocol_profile`
`v3-capable` and `protocol_versions` `[1,2,3]`; and the report is
`sley2-cli-report-v3` (section 3). The version 3 JSON renderings (the
table and the hello's `native_tests` feature key) are the bridge's
(`docs/spec/SMP1_JSON_BRIDGE_V1.md` revision 12, section 11).

## 10. Private native-test worker entry (revision 10)

`sley __native-test-worker <input_path>` exists so the native test
supervisor (`crates/sley-test-runner`, `docs/spec/NATIVE_TEST_ADMISSION_V1.md`)
can run its worker from the installed `sley` binary inside a transient
unit. It is not a user command, not a protocol method, and never appears
in the method table, the command list of section 1, or any
report. Its argv is exactly the one `render_transient_unit` renders after
the worker path: the word `__native-test-worker` and one absolute path to
the daemon-owned read-only input binding. Any other shape (no operand, a
relative path, an extra word) is `CLI_USAGE_INVALID` under section 4.

The worker opens the input path, reads exactly one length-delimited
`SLEYWRK1` request envelope, and answers on standard output with raw
refusal words: one big-endian `u32` refusal tag followed by the ASCII
detail code, with no newline and nothing on standard error. Its exit
status passes through unwrapped:

| Status | Tag | Detail | Meaning |
|---:|---:|---|---|
| 1 | 1 | the SCB registry string | malformed envelope |
| 6 | 2 | `NATIVE_WORKER_EXECUTION_NOT_WIRED` | well-formed envelope; execution is not wired (N5) |
| 7 | 3 | `NATIVE_WORKER_INPUT_UNREADABLE` | the input binding cannot be opened |
| 8 | — | — | the refusal words cannot be written |

These statuses are disjoint from the section 4 statuses (0, 2, 3, 4, 5),
so the launcher can tell a worker refusal from a CLI failure. A flush
failure after the worker ran is `CLI_IO_FAILURE` (status 4). The entry
links `sley-test-runner` (and through it `sley-vm`, `sley-scb1`,
`sley-tests`, `sley-id`), the one exception to the section 5 dependency
rule; the rule audit bounds it to this one call and one command word.
