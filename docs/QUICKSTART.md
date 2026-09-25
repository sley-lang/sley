# Sley Quickstart

This guide takes you from nothing to a working Sley 2 endpoint in about ten
minutes. You'll install the `sley` binary, check it, run the end-to-end demo,
and then drive a session by hand with a short Python client.

> **New to Sley?** Skim [Concepts](CONCEPTS.md) first. The one idea you need is
> that Sley has no source files. Programs are typed semantic graphs, and you
> work with them through a machine protocol (SMP1), not an editor.

**Contents**

1. [Install](#1-install)
2. [Check the binary](#2-check-the-binary)
3. [Run the end-to-end demo](#3-run-the-end-to-end-demo)
4. [Talk to `sley serve` yourself](#4-talk-to-sley-serve-yourself)
5. [Commands and exit codes](#5-commands-and-exit-codes)
6. [Run the test gates](#6-run-the-test-gates)
7. [Where to go next](#7-where-to-go-next)

---

## 1. Install

Sley 2.0.1 supports **Linux x86_64**. The release binary is statically linked
(musl), so it has no runtime dependencies.

### Option A: the release archive

Download `sley-2.0.1-linux-x86_64.tar.gz` from
[Releases](https://github.com/sley-lang/sley/releases), then verify and unpack
it:

```sh
sha256sum sley-2.0.1-linux-x86_64.tar.gz     # compare with the published SHA-256
tar xzf sley-2.0.1-linux-x86_64.tar.gz
cd sley-2.0.1-linux-x86_64
./bin/sley version
```

The archive contains:

| Path | What it is |
|---|---|
| `bin/sley` | The static `sley` binary |
| `demo/run_demo.py` | The self-contained end-to-end demo (Python 3, standard library only) |
| `conformance/` | The demo fixture plus the SMP1 and JSON-bridge conformance corpora |
| `MANIFEST.json` | Every member with its size and SHA-256 |
| `SBOM.json`, `LICENSES.json` | The dependency inventory and license inventory |
| `LICENSE`, `NOTICE` | Apache-2.0, the license of Sley itself |
| `THIRD_PARTY_LICENSES` | The license texts and copyright notices of the third-party crates compiled into the binary (new in 2.0.1) |

The release is reproducible: two clean builds, plus a rebuild on a second
host, produce the same bytes. The
[release notes](release/SLEY-2.0.1.md#build-and-verify) show how to check
that yourself.

### Option B: build from source

You need `git`, the Rust toolchain pinned in
[`rust-toolchain.toml`](../rust-toolchain.toml) (`rustup` installs it
automatically), and Python 3 for the demo.

```sh
git clone https://github.com/sley-lang/sley.git
cd sley
cargo build --release -p sley-cli
./target/release/sley version
```

A release build takes about a minute on a modern machine, plus the time to
download dependencies on the first run. To put `sley` on your `PATH`:

```sh
install -m 0755 target/release/sley ~/.local/bin/sley
```

The rest of this guide assumes `sley` is on your `PATH`. If it isn't,
substitute `./target/release/sley` or `./bin/sley`.

## 2. Check the binary

`sley` never prints prose. Every command writes machine-readable output, and
failures go to standard error as a single JSON object.

```console
$ sley version
{"cli":"1","contract":"sley2-cli-v1","protocol_version":1}
```

`sley hello --json` shows exactly what this endpoint offers an agent: the
protocol versions, the frozen schema epoch, the hard limits, and every method
it dispatches.

```console
$ sley hello --json
{
  "protocol_versions": [1],
  "schema_epochs": ["a7fcf97a85d41ef9b1c89394a324f2dc7ec875b9ded48a783104314857dc870e"],
  "features": {"cancel": true, "stream": true, "checksum": false, "json_bridge": false},
  "limits": {"max_depth": 65535, "max_entities": 65535, "max_edges": 400000,
             "max_frame_bytes": 67108864, "max_response_bytes": 67108864,
             "max_inflight": 1024, "max_sessions": 256, "max_work": 100000000},
  "adapters": [], "effects": [],
  "methods": ["session.open", "workspace.create", "refs.list", "branch.create",
              "compare", "merge.judge", "exchange.export", "exchange.import",
              "query.root", "capsule", "candidate.create", "candidate.validate",
              "commit", "execute", "report", "…"]
}
```

*(Pretty-printed and shortened here. The real output is one line.)*

`"json_bridge": false` doesn't mean the JSON form is missing. `--json` is a
transport choice the `sley` CLI makes for its own input and output, not a
feature the two sides negotiate. The offer never carries a transport feature,
so byte mode and JSON mode negotiate the same profile and the same session
identity. [SLEY_CLI_V1](spec/SLEY_CLI_V1.md#2-serve) section 2 gives the
reasoning.

`sley methods` prints the full method table, grouped by family: `session`,
`repository`, `query`, `candidate`, `transaction`, and `runtime`. Each method
has a stable numeric tag.

```console
$ sley methods | jq -r '.methods[:6][] | "\(.tag)  \(.family)\t\(.name)"'
100  session	session.open
101  session	session.renew
102  session	session.close
103  session	session.capabilities
104  session	session.budgets
200  repository	workspace.create
```

## 3. Run the end-to-end demo

The release demo exercises the whole stack through the `sley` binary alone.
It needs no source tree and no network access. It works in two empty
directories:

1. It **imports** a small executable program into the first repository.
2. It runs a **root query** and checks the answer against the expected bytes.
3. It **executes** a function in the deterministic VM and reads back the
   stored execution **report**.
4. It **creates a branch**, **exports** the repository, and runs a **GC dry
   run**.
5. It **clones** the export into the second repository and checks that the
   query and the execution give byte-identical results there.

From a source checkout:

```sh
python3 bench/release/run_demo.py \
  --sley target/release/sley \
  --fixture conformance/release-demo/v1/demo.json
```

From the unpacked release archive:

```sh
python3 demo/run_demo.py
```

The archive's demo finds its binary and fixture next to itself, so you can
also run it from any directory by its path. It works in a temporary directory
and removes it when it finishes. Pass `--work <dir>` to keep the two
repositories for inspection.

The run ends with:

```json
{
  "result": "PASS",
  "problems": [],
  "explicit_gap": "candidate construction, commit, test selection, and merge wait for the public candidate builder (S20-350 proposal-only)",
  "steps": {
    "query_root_matches_fixture": true,
    "execute_report_id_matches_fixture": true,
    "execute_response_matches_fixture": true,
    "report_answers_stored_record": true,
    "branch_created": true,
    "exported_bytes": 9430,
    "gc_dry_run_no_candidates": true,
    "clone_query_root_matches_fixture": true,
    "clone_execute_matches": true,
    "clone_gc_dry_run_no_candidates": true,
    "first_endpoint_exit": 0,
    "second_endpoint_exit": 0
  }
}
```

*(Shortened here. The real output also carries the demo's contract, the
handshake identity, the environment it ran in, and the CLI version.)*

Every answer is compared byte for byte against the fixture, so a `PASS` means
your build computes exactly the expected bytes for these requests.

The `explicit_gap` field states what the demo does not cover. It imports a
program that was built in advance. It does not construct a candidate, commit
one, select tests, or merge through the public candidate builder, because the
public builder is still proposal-only. Those paths are exercised by the test
suites and conformance corpora, not by this demo.

## 4. Talk to `sley serve` yourself

`sley serve` is the endpoint an agent talks to. It reads SMP1 frames on
standard input and writes answers on standard output. With `--json`, each
frame is one JSON object per line, which makes it easy to drive from any
language.

```sh
sley serve --repository ./my-repo --json [--report report.json]
```

The repository directory is created on first write. It holds the object
store, refs, branches, heads, and transaction receipts.

### The shape of a frame

Every request is one JSON object on one line:

```json
{
  "kind": "request",
  "method": "query.root",
  "protocol_version": 1,
  "request_id": 1,
  "session": "<session id from session.open>",
  "body": "<canonical request record, hex-encoded>",
  "bounds": { "applied_limits": { "max_depth": 0, "...": 0 }, "truncated": false, "...": 0 },
  "flags": { "cancel": false, "failed": false, "stream": false }
}
```

- **`body`** is the method's canonical binary request record, hex-encoded. SMP1 owns the
  transport and nothing else. Payloads are the frozen records of each
  subsystem's contract, listed in the appendices of
  [SMP1](spec/SMP1.md#appendix-a-body-records-of-the-dispatched-methods-s20-410).
- **`request_id`** is `0` for session-less requests (the handshake,
  `exchange.import`, and `session.open`). Inside a session it counts upward
  from `1`.
- **`bounds`** is the bounded-context record. Requests send zeros. Every
  response fills it in with the negotiated limits it was fitted against and
  the counts it returned, omitted, or truncated. If a response wouldn't fit,
  it fails with `PROTOCOL_LIMIT_EXCEEDED` rather than arriving cut short.
- A failed answer sets **`flags.failed`**, and its body carries a stable
  failure record. A failed answer is not a CLI failure, so the process keeps
  running.

### A complete session in Python

[`docs/examples/smp1_json_client.py`](examples/smp1_json_client.py) is a
dependency-free client. It performs the core of the demo by hand:

```console
$ python3 docs/examples/smp1_json_client.py target/release/sley
hello        -> hello
import       -> ok
session.open -> cfec9864196a85ba…
query.root   -> matches fixture
execute      -> matches fixture
exit status  -> 0
```

The session id you see will differ from this one, and it changes on every
run. Each endpoint instance mints session ids over its own per-instance nonce
(see `session.open` in [SMP1](spec/SMP1.md)), so a session id is never
reusable across runs. The other lines are the same every time.

Here's the conversation it has with the endpoint:

```text
client                                   sley serve --json
  │── hello (the offer from `sley hello`) ──▶│
  │◀──────────────── hello (negotiated) ─────│   profile selected
  │── exchange.import  [id 0, no session] ──▶│
  │◀────────────────────────────── ok ───────│   program stored, head set
  │── session.open     [id 0, handshake] ───▶│
  │◀────────────────────── session id ───────│
  │── query.root       [id 1, session] ─────▶│
  │◀─────────────────── root summary ────────│   bounded, typed, exact
  │── execute          [id 2, session] ─────▶│
  │◀──────────── execution response ─────────│   deterministic VM + stored report
  │── session.close    [id 3, session] ─────▶│
  │── (end of input) ───────────────────────▶│   exit 0
```

Three details matter when you write your own client:

1. **The first frame must be a hello.** Get the offer with
   `sley hello | sley frame decode`, which renders the binary hello frame as
   a JSON frame line. Any other first frame, including a first line that
   isn't a valid frame, or no input at all, fails with
   `CLI_HANDSHAKE_REQUIRED` (exit 5).
2. **The handshake identity is deterministic.** `session.open` takes it as
   its body. The example gets it from a one-frame probe run with `--report`.
   The report records `handshake_id` along with frame, answer, and
   failure-code counters.
3. **Event frames can come before a response.** Streaming methods emit events
   first, so read lines until one has `kind` equal to `response`.

### Binary frames

Without `--json`, `serve` speaks raw SMP1: an 8-byte length prefix followed
by the frame bytes. `sley frame decode` and `sley frame encode` convert
between the two forms, which helps when you debug a binary client:

```sh
sley hello | sley frame decode          # binary hello  → JSON line
echo "$json_line" | sley frame encode   # JSON line     → binary frame
```

## 5. Commands and exit codes

```text
sley version                 print CLI and protocol identity
sley hello [--json]          print the hello this endpoint offers
sley methods                 print the method table
sley frame decode|encode     convert SMP1 frames: binary ⇄ JSON lines
sley serve --repository <dir> [--json] [--batch] [--report <file>]
```

`--protocol-profile v2-capable|v3-capable` opts into protocol version 2
(bounded `entity.version` and `entity.signature` reads) and version 3 (native
test admission).
Arguments are exact: there are no abbreviations, environment variables, or
config files. The full contract is in [SLEY_CLI_V1](spec/SLEY_CLI_V1.md).

| Exit | Symbol | Code | Meaning |
|---:|---|---:|---|
| 0 | — | — | Ran to end of input and answered every frame, including failed answers |
| 2 | `CLI_USAGE_INVALID` | 43000 | Unknown command, flag, or value |
| 3 | `CLI_INPUT_INVALID` | 43001 | `serve` in byte mode: a truncated length prefix or frame body. `frame decode` and `frame encode`: input they can't convert |
| 4 | `CLI_IO_FAILURE` | 43002 | Read, write, or flush failure |
| 5 | `CLI_HANDSHAKE_REQUIRED` | 43003 | The first frame wasn't a client hello, or there was no input |

A CLI failure writes one JSON object to standard error:

```console
$ sley --help; echo "exit $?"
{"cause":"--help","code":43000,"symbol":"CLI_USAGE_INVALID"}
exit 2
```

In `--json` mode a malformed line is not a CLI failure. After the handshake,
the endpoint answers a line it can't parse with a failed response frame
(`flags.failed` set, request id `0`, no session) that carries the bridge's
code, such as `42000 JSON_BRIDGE_SHAPE_INVALID`, and keeps reading. The run
still exits `0`. Two cases differ:

- A malformed **first** line is a missing hello, so it exits `5`:

  ```console
  $ echo 'not json' | sley serve --repository ./r --json; echo "exit $?"
  {"cause":"JSON_BRIDGE_SHAPE_INVALID","code":43003,"symbol":"CLI_HANDSHAKE_REQUIRED"}
  exit 5
  ```

- A line over one of the bridge's ceilings is answered with
  `JSON_BRIDGE_RESOURCE_LIMIT` and ends the input, because the endpoint
  can't resynchronise with a producer that overran a ceiling.

To catch malformed lines in `--json` mode, check `flags.failed` on each
response, or run with `--report` and read its `failed_answers` and `codes`
counters.

Kernel failures (type errors, refused candidates, stale preconditions, and so
on) travel inside failed answers as stable codes. The registry is
[ERROR_CODES_V1](spec/ERROR_CODES_V1.md).

## 6. Run the test gates

These are the gates a contributor can run from a fresh clone:

```sh
cargo test --workspace --locked   # every crate's unit and integration tests
make conformance                  # Rust vs. the independent Python oracle (needs uv)
make adversarial                  # corruption, crash-recovery, and binding-confusion suites
make fuzz-smoke                   # bounded fuzz smoke across codecs and importers
make lint                         # rustfmt check, plus clippy with all + pedantic denied
```

Set aside time for the first run. A cold `cargo test --workspace` compiles and
runs every crate's suites, and takes about 15 to 20 minutes on a typical
machine. Later runs reuse the build and are much faster.

`make lint` records its result in the tracked file
`evidence/build/lint-report.json`, and that record names the current commit,
so running it leaves the working tree dirty. If you only wanted the check,
discard the record afterwards:

```sh
git checkout -- evidence/build/lint-report.json
```

### Reproduce the release artifact

To reproduce the release artifact exactly, you need a clean checkout at the
release commit, the `x86_64-unknown-linux-musl` target, and `uv`. The build
reads crate sources offline, for the SBOM and the third-party license texts,
so on a machine whose cargo cache hasn't seen this lockfile yet, fetch every
locked dependency first:

```sh
rustup target add x86_64-unknown-linux-musl
cargo fetch --locked              # needed on a fresh cargo cache, harmless otherwise
make release-candidate-smoke      # two clean musl builds, compared member by member, then verified
sha256sum dist/sley-2.0.1-linux-x86_64.tar.gz
```

The build refuses a dirty working tree, so if you ran `make lint`, commit or
discard its record first. The smoke rewrites the tracked release records
under `evidence/` and `machineresearch/`, so expect a dirty tree when it
finishes.

### The maintainers' gate

`make quick` is the maintainers' routine gate, and it doesn't pass on a fresh
clone. It needs two things a contributor usually won't have:

- **A local release build.** Some of its checkers read the outputs that
  `make release-candidate-build` writes to `evidence/runtime/`, which isn't
  tracked.
- **The Sley 2.0 master goal.** Two of its checkers read this planning
  document, which lives outside the repository. Maintainers point
  `SLEY2_MASTER_GOAL` at it.

Contributors can run the gates listed above instead. If your change touches a
spec or a checker, also run the matching `scripts/check_*.py` directly. Most
of them read only the tracked tree.

## 7. Where to go next

| Next | What you'll find |
|---|---|
| 🧠 [Concepts](CONCEPTS.md) | The model behind the protocol: identities, roots, candidates, and receipts |
| 🧭 [Architecture walkthrough](https://sleylang.org/tutorial) | From verified state to proposal, validation, transaction, and new state, step by step |
| 📡 [SMP1](spec/SMP1.md) | The full protocol: framing, handshake, sessions, bounds, cancellation, and every body record |
| 🔁 [JSON bridge](spec/SMP1_JSON_BRIDGE_V1.md) | The exact JSON mapping that `--json` uses |
| 📚 [Documentation index](README.md) | Every spec, grouped by topic |
| 𝕏 [@SleyLanguage](https://x.com/SleyLanguage) | Release news and updates |
