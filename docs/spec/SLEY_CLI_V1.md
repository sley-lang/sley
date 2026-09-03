# Thin Machine-Oriented CLI v1

Status: S20-430 contract draft, revision 1 (2026-09-03); Council review
pending (Ariadne contract review, Nabu architecture review, Vulcan surface
review). No implementation exists at this revision. Implementation state is
tracked in the machine summary.

The CLI is a transport endpoint and nothing else. It moves SMP1 frames
between standard input, standard output, and the deterministic S20-410
server over one repository path, in either the canonical byte form or the
S20-420 JSON form, and it writes a machine-readable invocation report. It
owns no semantics: every judgment about a frame comes from the server
(`docs/spec/SMP1.md` revision 5, S20-440 batch admission, S20-330
sessions) and every representation from the frozen codec or the bridge
(`docs/spec/SMP1_JSON_BRIDGE_V1.md`). The master goal requires a thin
machine-oriented wrapper that contains no private validation rules and that
the semantic kernel never imports (master goal sections 14.2, 14.3, 22.6).

## 1. Commands

```text
sley serve --repository <path> [--json] [--batch] [--report <path>]
sley frame decode                       # stdin: frames as bytes; stdout: one Frame object per line
sley frame encode                       # stdin: one Frame object per line; stdout: frames as bytes
sley methods                            # stdout: the generated method table
sley hello [--json]                     # stdout: the hello this endpoint offers, as a hello frame
sley version                            # stdout: {"cli":"1","protocol_version":1,"contract":"sley2-cli-v1"}
```

Arguments are exact: an unknown command, a repeated or unknown option, or
a missing value is `CLI_USAGE_INVALID`. There are no abbreviations, no
environment variables, no configuration files, and no prose output on any
stream.

## 2. `serve`

The endpoint offers the hello of the deterministic server
(`Server::offered_hello`): protocol version 1, the frozen conformance
schema epoch, the limit ceilings, every method the server dispatches
(reserved and deferred methods are not offered), the cancel and stream
features, and no adapters or effects; under `--json` the endpoint adds the
`json_bridge` feature. The first frame read must be the client hello
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
that many bytes; the bytes are handed to the server unchanged. A prefix
above the ceiling is answered with the codec's `PROTOCOL_FRAME_TOO_LARGE`
without reading the body. A short read inside a frame is
`CLI_INPUT_INVALID`. JSON mode reads one `Frame` object per line
(terminated by `\n`), converts it with `frame_from_json`, and writes one
`Frame` object per line with `frame_to_json`; a line the bridge rejects is
answered with a response frame carrying the bridge's own code and symbol
(request identifier zero, no session) and reading continues. End of input
ends the invocation; the endpoint never waits for a close.

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
the numeric code of every failed answer's `ProtocolFailure` body; `cause`
carries the underlying codec or bridge symbol when a CLI failure wraps one.

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
error.

## 5. Rules audited mechanically

`scripts/check_cli_rules.py` fails closed when any of these drifts:

- `crates/sley-cli` depends only on `sley-protocol`, `sley-json-bridge`,
  and `serde_json` (development dependencies may add `sley-repo` with
  `test-support`, `sley-id`, and `sley-scb1` for fixtures);
- no other workspace crate depends on `sley-cli`;
- the CLI source names no kernel crate (`sley_ssmc`, `sley_check`,
  `sley_query`, `sley_mutate`, `sley_txn`, `sley_repo`, `sley_policy`,
  `sley_vm`, `sley_state_root`, `sley_store`, `sley_adapter`,
  `sley_schema`, `sley_scb1`) outside its tests;
- the CLI source constructs no `ProtocolFailure {` literal, no owner
  record, and no `ProtocolFrame {` literal other than the handshake
  failure frame, and calls `encode_frame` only there;
- the CLI source contains no `fn validate`, `fn judge`, `fn check_`, no
  method-name or method-tag match arms, and no text output that is not a
  frame, a report, or the version object.

## 6. Required evidence

- Native tests over a trusted genesis repository: the handshake and a
  session opened through standard input in byte mode and in JSON mode with
  byte-identical responses to a direct `Server` over the same repository;
  the batch mode cancelling a request before execution; the negotiation
  failure response; every CLI failure with its exit status and stderr
  object; the report's counts against the frames written; `frame decode`
  and `frame encode` reproducing the S20-420 fixture; `methods`, `hello`,
  and `version` outputs.
- `scripts/check_cli_rules.py` in `make quick`.
- Tier 1 plus Tier 2 validation, and the Ariadne, Nabu, and Vulcan
  reviews with every report-grade finding closed.

## 7. Explicit exclusions

This contract does not claim: network transport, authentication, or
multi-tenant isolation; a daemon or socket server; interactive use, prompts,
colour, or prose; command-count parity with any legacy CLI (master goal
section 15.2); the benchmark harness (`sley-bench`); runtime, packaging,
release, or GA.
