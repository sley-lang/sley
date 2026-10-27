<p align="center">
  <img src="assets/branding/canonical/sley_2_0_loom_graph_banner_1500x500.png" alt="Sley 2.0" width="100%">
</p>

<h3 align="center">Machine-native programming for AI agents.</h3>

<p align="center">
  Machines don't write source. They change verified program state.
</p>

<p align="center">
  <a href="https://sleylang.org"><img alt="Website" src="https://img.shields.io/badge/sleylang.org-website-22d3ee?style=flat-square"></a>
  <img alt="Version 2.1.6" src="https://img.shields.io/badge/version-2.1.6-38c8e8?style=flat-square">
  <a href="LICENSE"><img alt="License: Apache-2.0" src="https://img.shields.io/badge/license-Apache--2.0-3b82f6?style=flat-square"></a>
  <img alt="Rust 1.93" src="https://img.shields.io/badge/rust-1.93-f97316?style=flat-square&logo=rust&logoColor=white">
  <img alt="Linux x86_64" src="https://img.shields.io/badge/platform-linux%20x86__64-64748b?style=flat-square&logo=linux&logoColor=white">
  <a href="https://x.com/SleyLanguage"><img alt="Follow @SleyLanguage on X" src="https://img.shields.io/badge/@SleyLanguage-000000?style=flat-square&logo=x&logoColor=white"></a>
</p>

<p align="center">
  <a href="docs/QUICKSTART.md"><b>Quickstart</b></a> ·
  <a href="docs/CONCEPTS.md"><b>Concepts</b></a> ·
  <a href="https://sleylang.org/tutorial"><b>Walkthrough</b></a> ·
  <a href="docs/README.md"><b>Documentation</b></a> ·
  <a href="docs/release/SLEY-2.1.6.md"><b>Release notes</b></a> ·
  <a href="https://sleylang.org/faq"><b>FAQ</b></a> ·
  <a href="https://sleylang.org/arena"><b>Arena</b></a>
</p>

---

## What is Sley?

Sley is a programming system built for AI agents rather than for people
typing into text editors. Sley programs aren't source files. A program is a
**typed, immutable semantic graph**: functions, types, blocks, operations,
contracts, and tests stored as content-addressed objects. An agent reads that
graph through bounded queries and changes it by proposing typed mutations. A
deterministic kernel checks every proposal before anything is committed.

```text
verified state ─▶ query ─▶ proposal ─▶ validation ─▶ commit admission ─▶ atomic commit
```

Every accepted state has an exact cryptographic root. Accepted transactions
carry receipts, and supported refusals use stable machine codes.

"Verified" names checks under a specific profile and policy. Static validity,
advisory test matches, resource evidence and accepted state are separate;
none alone establishes correctness against user requirements. The
[guarantee matrix](docs/GUARANTEES.md) explains what each result means.

## Why Sley?

| When an agent writes source text… | With Sley… |
|---|---|
| It emits characters and hopes they parse. | It proposes **typed mutations** over a closed schema. Malformed input is rejected at the boundary. |
| Context means pasting whole files into a prompt. | Context is a **bounded, typed query** with explicit limits and no silent truncation. |
| "Is this valid?" depends on toolchains and flags. | One **deterministic kernel** judges types, control flow, effects, contracts, and policy the same way every time. |
| Identity is a file path and line number. | Identity is **content-addressed**: entities, objects, state roots, and transactions all have exact digests. |
| A half-applied edit can corrupt a tree. | Commits are **atomic**. After a crash you have either the old state or the complete new one. |
| Errors are paragraphs to interpret. | Errors are **stable numeric and symbolic codes** with exact contracts. |

## Quickstart

> **Requirements:** Linux x86_64. To build from source you also need the Rust
> toolchain pinned in [`rust-toolchain.toml`](rust-toolchain.toml) (1.93.0).
> The protocol demo needs Python 3.

**1. Get the `sley` binary.** Download the release archive from
[Releases](https://github.com/sley-lang/sley/releases), check it, and put its
`bin/` directory on your `PATH`:

```sh
curl -fLO https://github.com/sley-lang/sley/releases/download/v2.1.6/sley-2.1.6-linux-x86_64.tar.gz
curl -fLO https://github.com/sley-lang/sley/releases/download/v2.1.6/SHA256SUMS
sha256sum -c SHA256SUMS
tar xzf sley-2.1.6-linux-x86_64.tar.gz
export PATH="$PWD/sley-2.1.6-linux-x86_64/bin:$PATH"
sley version
```

Or build it from source and put `target/release/` on your `PATH` instead:

```sh
git clone --branch v2.1.6 --depth 1 https://github.com/sley-lang/sley.git
cd sley
cargo build --release -p sley-cli -p sley-agent   # → target/release/sley, sley-agent
```

`--branch v2.1.6` builds the released source. `main` carries unreleased work
under the same workspace version, so omit it only when you want that work.

### Write and test your first program

**2. Write a function in Sley's Python dialect.** `sley-agent` compiles typed
Python into a Sley candidate, validates it, and runs its tests. Save this as
`area.py` in an empty directory:

```python
def area(w: int, h: int) -> int:
    return w * h

def test_area():
    assert area(3, 4) == 12
    assert area(0, 9) == 0
    assert area(5, 5) == 25
```

Then, from that directory:

```console
$ sley-agent init demo && cd demo
$ sley-agent try --familiar ../area.py
c1: 3 passed (advisory) d1@r1
$ sley-agent call area 3 4 --on c1
12
$ sley-agent submit d1
submitted c1 from d1@r1 -> final_candidate.hex (2787-byte candidate, hex); resubmit any time, the last submission wins
```

Use the candidate and draft handles printed by your own `try` command if
they differ. `sley-agent submit d1` writes `final_candidate.hex`; it does
not change the accepted workspace head.

**Released 2.1.6 boundary:** these are advisory test matches. Committing
this candidate, which changes both code and its targeting tests, refuses
with `TXN_TEST_EVIDENCE_UNSUPPORTED` until supported native admission is
available. Validation, advisory execution, submission and authoritative
commit are separate outcomes.

`sley-agent help python` lists what the dialect accepts. `sley-agent help`
is the full guide (under 8 KiB), including the JSON authoring form, and the
[Quickstart authoring section](docs/QUICKSTART.md#7-write-and-test-a-program-with-sley-agent)
walks through inspection and submission.

### Inspect the protocol and imported fixtures

The following commands explore the lower-level endpoint and a program built
in advance. They are a separate protocol demonstration from writing the
new candidate above. Run source-checkout commands from the checkout root.

**3. Explore the protocol.** Every `sley` command prints machine-readable JSON:

```console
$ sley version
{"cli":"1","contract":"sley2-cli-v1","protocol_version":1}

$ sley hello --json
{"adapters":[],"effects":[],"features":{"cancel":true,"checksum":false,"json_bridge":false,"stream":true},
 "limits":{"max_depth":65535,"max_entities":65535,"max_frame_bytes":67108864,...},
 "methods":["session.open","workspace.create","query.root","candidate.create","commit","execute",...],
 "protocol_versions":[1],"schema_epochs":["a7fcf97a…"]}
```

`"json_bridge": false` is expected. `--json` is a transport choice of the
CLI, not a feature the endpoint negotiates
([SLEY_CLI_V1](docs/spec/SLEY_CLI_V1.md#2-serve) section 2).

**4. Run the prebuilt protocol demo.** It imports a program into an empty repository,
queries it, executes a function, stores the report, creates a branch, exports
the repository, clones it into a second empty directory, and checks that the
clone gives byte-identical answers:

```console
$ python3 bench/release/run_demo.py --sley target/release/sley \
    --fixture conformance/release-demo/v1/demo.json
{
  "result": "PASS",
  "explicit_gap": "candidate construction, commit, test selection, and merge wait for the public candidate builder …",
  "steps": {
    "query_root_matches_fixture": true,
    "execute_response_matches_fixture": true,
    "report_answers_stored_record": true,
    "branch_created": true,
    "exported_bytes": 9430,
    "clone_query_root_matches_fixture": true,
    "clone_execute_matches": true,
    ...
  }
}
```

From the release archive, run `python3 demo/run_demo.py` inside the unpacked
directory. No source tree is needed.

The demo imports and executes a program that was built in advance. As
`explicit_gap` says, it doesn't construct, commit, test-select, or merge a
candidate through the public builder, which is still proposal-only.

➡️ **The [full Quickstart](docs/QUICKSTART.md)** covers verifying the
download, driving `sley serve` by hand over JSON lines, exit codes, and the
full test gates.

## Play in the Sley Arena

[Sley Arena](https://sleylang.org/arena) is a bot league for Sley programs.
Bots are written in the same Python dialect, weave for territory on a shared
loom, and climb a public ladder. Hand your coding agent the
[player guide](https://arena-api.greyforge.tech/agent.md), or start from the
one-file kit (Node.js 18 or newer):

```sh
curl -fsSLO https://arena-api.greyforge.tech/kit/sley-arena.mjs
node sley-arena.mjs init    # writes bot.py, a working starter bot
node sley-arena.mjs test    # duels against every practice bot
```

The [rules](https://sleylang.org/arena/rules), the ladder and match replays
are at [sleylang.org/arena](https://sleylang.org/arena).

## How it works

```mermaid
flowchart LR
    A([Agent]) -- "SMP1 frames<br/>(binary or JSON lines)" --> P[sley serve]
    P --> Q[Queries and capsules]
    P --> M[Mutation candidates]
    M --> C{Checker and policy<br/>14-phase validation}
    C -- valid --> G{Commit admission}
    G -- admitted --> T[Transaction engine]
    G -- refused --> R
    C -- refused --> R[/Stable error code/]
    T --> S[(Content-addressed<br/>object store)]
    T --> RC[/Receipt + new state root/]
    P --> V[Deterministic VM]
    V --> S
    Q --> S
```

Sley has three canonical layers:

| Layer | What it is | Spec |
|---|---|---|
| **SSMC1**: Sley Semantic Machine Code | The only canonical program form: a typed, immutable graph of 18 entity kinds (`Function`, `Block`, `Operation`, `TypeDef`, `Contract`, `TestCase`, …). | [`docs/spec/SSMC1.md`](docs/spec/SSMC1.md) |
| **SCB1**: Sley Canonical Binary | The only canonical byte encoding. Strict and minimal. Malformed input is rejected, never normalized. BLAKE3 with domain-separated hashes. | [`docs/spec/SCB1.md`](docs/spec/SCB1.md) |
| **SMP1**: Sley Machine Protocol | The programming interface: versioned, bounded, cancellable request/response frames, with a JSON bridge for convenience. | [`docs/spec/SMP1.md`](docs/spec/SMP1.md) |

Read **[Concepts](docs/CONCEPTS.md)** for the full model: identities, state
roots, the change lifecycle, sessions, and the glossary.

## Recent releases

- [2.1.6](docs/release/SLEY-2.1.6.md): deeply nested source gets a diagnostic with its line and column, nested built-in calls parse in linear time, and the `sley_agent::embed` API runs Sley inside Rust hosts and WebAssembly ([embedding guide](docs/EMBEDDING.md)).
- [2.1.5](docs/release/SLEY-2.1.5.md): the VM's word tier runs all-integer programs on plain 64-bit registers; `try` prints only failures; `file:line:col` diagnostics with a caret; `struct` and `enum` in the Python dialect.
- [2.1.4](docs/release/SLEY-2.1.4.md): faster loops, plus pytest-style tests and `@dataclass` records in the Python dialect.
- [2.1.3](docs/release/SLEY-2.1.3.md): recursive and call-heavy programs run in the VM's compact scalar plan.
- [2.1.2](docs/release/SLEY-2.1.2.md): tuple assignment to parameters and clearer test-limit reporting.
- [2.1.1](docs/release/SLEY-2.1.1.md): write Sley in a typed Python dialect with `sley-agent try --familiar`.
- [2.1.0](docs/release/SLEY-2.1.0.md): Sley builds Sley. The toolchain is a Sley program that rebuilds itself from its canonical graph to a byte-identical fixed point; reproduce it with `scripts/sh2_bootstrap.sh`.
- [2.0.0](docs/release/SLEY-2.0.0.md): the semantic kernel, repository, protocol, CLI and reproducible packaging, with fixes in [2.0.1](docs/release/SLEY-2.0.1.md) and the agent workbench in [2.0.2](docs/release/SLEY-2.0.2.md).

## What Sley 2 includes

- **A deterministic semantic kernel.** Type system, control-flow and
  value-use validation, exact effect closure, contracts and tests, semantic
  fingerprints, and impact analysis.
- **A content-addressed repository.** An immutable object store, deterministic
  state roots, atomic transactions with receipts, native branches, semantic
  compare and merge (ambiguity becomes explicit conflict objects), repository
  exchange/clone, garbage collection, and crash recovery.
- **Agent-shaped context.** Root-backed typed queries and context capsules with
  hard limits. They fail loudly instead of truncating silently.
- **Proposal-first mutation.** Generated mutation descriptors for every entity
  kind and field. Candidates are validated in 14 ordered phases before any
  state moves.
- **A deterministic VM.** Bounded execution with fuel, output, and cancellation
  limits. It produces canonical observation digests and stored execution
  reports.
- **Policy and capabilities.** Protected policy roots, scoped capability
  tokens, and bounded adapters. There is no arbitrary shell.
- **A reproducible release.** Two clean musl builds compared member by member,
  CycloneDX and SPDX SBOMs, and recorded (unsigned) provenance. Each release's
  notes give its reproducibility record.
- **Independent conformance.** A separate Python oracle reproduces the Rust
  kernel's bytes exactly, backed by persistent fuzz targets across ten
  surfaces.

The **[2.0.0 release notes](docs/release/SLEY-2.0.0.md)** have the complete
list, including the known limits.

## Project status

**Sley 2.1.6** is the current release; [Recent releases](#recent-releases)
lists what each point release added. The
[2.0.0 release notes](docs/release/SLEY-2.0.0.md#known-limits-and-what-is-not-yet-claimed)
list the acceptance criteria and findings still open from 2.0.0.

The [initial workload and adoption roadmap](docs/ADOPTION_ROADMAP.md) focuses
evaluation on short function authoring, then on separately measured changes to
existing programs.

| Track | State |
|---|---|
| **Sley 2.0** | Released as 2.0.0, with fixes in 2.0.1 and the agent workbench in 2.0.2. The kernel, repository, protocol, CLI, agent workbench, and reproducible packaging are in place. |
| **Sley 2.1** | Released as 2.1.0, with point releases through 2.1.6 ([Recent releases](#recent-releases)). **Self-hosted**: the Sley toolchain builds itself, seed absent, to a byte-identical fixed point ([ADR-0056](docs/adr/ADR-0056-sh2-self-hosted-toolchain.md), [Bootstrap Profile 3](docs/spec/BOOTSTRAP_PROFILE_3.md), [reproduce it](docs/release/SLEY-2.1.0.md#reproduce-it)). |
| **Succession benchmark** | In progress. It measures agents working in Sley against raw source and Sley 1.x on a frozen 15-task corpus. Results will be published when the campaign finishes. |
| **Sley 1.x** | Frozen at [v1.2.1](https://github.com/GreyforgeLabs/sley-legacy/tree/v1.2.1), the final 1.x tag, in [GreyforgeLabs/sley-legacy](https://github.com/GreyforgeLabs/sley-legacy). It's a separate, human-readable language that is intentionally incompatible with 2.x. |

## Documentation

| Guide | What it covers |
|---|---|
| 🚀 **[Quickstart](docs/QUICKSTART.md)** | Install, verify, run the demo, and talk to `sley serve` by hand |
| 🧠 **[Concepts](docs/CONCEPTS.md)** | The mental model: SSMC1, SCB1, SMP1, identities, and the change lifecycle |
| 🧭 **[Architecture walkthrough](https://sleylang.org/tutorial)** | Step by step from verified state to proposal, validation, transaction, and new state |
| 📚 **[Documentation index](docs/README.md)** | Every specification, ADR, and reference, grouped by topic |
| 🏗️ **[Architecture](ARCHITECTURE.md)** | Crate authority, the dependency law, and the durability order |
| 📦 **[Release notes](docs/release/SLEY-2.1.6.md)** | The current release, how to verify it, and what isn't claimed. [2.1.0](docs/release/SLEY-2.1.0.md) is the self-hosted release; [2.0.0](docs/release/SLEY-2.0.0.md) lists the known limits that still apply. |
| 🔐 **[Security](SECURITY.md)** | Threat model, reporting, and the [threat register](docs/THREAT_REGISTER.md) |
| 🤝 **[Contributing](CONTRIBUTING.md)** | The slice contract, validation gates, and commit discipline |

## Build and test from source

With the pinned Rust toolchain (including rustfmt and Clippy), Python 3.12+
and `uv` installed, run the combined contributor gate from a fresh clone:

```sh
make contributor-check
```

It checks formatting, strict workspace Clippy, all workspace tests,
conformance, adversarial tests and bounded fuzz smoke, stopping on failure.
It uses tracked fixtures and locked dependencies; it needs no generated
release evidence, credentials or installed native supervisor.
The first run may download dependencies and creates ignored build/cache files,
but does not rewrite tracked evidence. This is source validation, not release
or native-host qualification.

The individual commands remain available:

```sh
cargo build --release -p sley-cli     # the sley binary
cargo test --workspace --locked       # every crate's tests (about 15 to 20 minutes cold)
make conformance                      # Rust vs. the independent Python oracle (needs uv)
make adversarial                      # corruption, crash, and binding-confusion suites
make fuzz-smoke                       # bounded fuzz smoke across the codecs and importers
make lint                             # rustfmt check, plus clippy with all + pedantic denied
```

`make lint` rewrites the tracked `evidence/build/lint-report.json`, so it
leaves the tree dirty. Discard the record with
`git checkout -- evidence/build/lint-report.json` if you only wanted the check.

To reproduce the release artifact you need a clean tree at the release
commit, the `x86_64-unknown-linux-musl` target, and `uv`. If you ran
`make lint`, commit or discard its record first. On a fresh cargo cache, fetch
the locked dependencies before the build:

```sh
cargo fetch --locked
make release-candidate-smoke          # two clean musl builds + every release record, then verify
```

Maintainers run `make quick` as the routine gate. It is not a fresh-clone
gate: it reads untracked inputs, including the outputs of a local
`make release-candidate-build` under `evidence/runtime/`. The
[Quickstart](docs/QUICKSTART.md#6-run-the-test-gates) has the details.

The repository is a Cargo workspace of 23 crates under [`crates/`](crates/).
Each crate owns exactly one authority, from `sley-scb1` (canonical bytes) to
`sley-vm` (execution) and `sley-cli` (the thin wrapper). See
[ARCHITECTURE.md](ARCHITECTURE.md) for the dependency law.

## Community and links

| Where | Link |
|---|---|
| 🌐 **Website** | [sleylang.org](https://sleylang.org) |
| 🏟️ **Sley Arena** | [sleylang.org/arena](https://sleylang.org/arena) · [player guide](https://arena-api.greyforge.tech/agent.md) |
| 📖 **Docs & walkthrough** | [sleylang.org/docs](https://sleylang.org/docs) · [sleylang.org/tutorial](https://sleylang.org/tutorial) · [sleylang.org/faq](https://sleylang.org/faq) |
| 🤖 **For LLMs** | [sleylang.org/llms.txt](https://sleylang.org/llms.txt) |
| 𝕏 **Sley on X** | [@SleyLanguage](https://x.com/SleyLanguage) |
| 𝕏 **Greyforge on X** | [@GreyforgeLabs](https://x.com/GreyforgeLabs) |
| 🔥 **Greyforge Labs** | [greyforge.tech](https://greyforge.tech) · [About](https://greyforge.tech/about) |
| 📰 **Background** | [The machine-native break](https://greyforge.tech/chronicles/sley-120-machine-native-break), on why Sley left source code behind |
| 🐙 **GitHub** | [sley-lang](https://github.com/sley-lang) · [GreyforgeLabs](https://github.com/GreyforgeLabs) |
| 🏛️ **Sley 1.x (legacy)** | [GreyforgeLabs/sley-legacy](https://github.com/GreyforgeLabs/sley-legacy) · [sleylang.org/legacy](https://sleylang.org/legacy) |

**Following Sley?** [Star sley-lang/sley](https://github.com/sley-lang/sley) and
follow [@SleyLanguage](https://x.com/SleyLanguage) for each release.

## License

Sley 2 is licensed under the [Apache License 2.0](LICENSE).
Copyright © 2026 [Greyforge Labs](https://greyforge.tech). See [NOTICE](NOTICE)
for details. Third-party dependency licenses are listed in the release SBOM,
and from 2.0.1 their texts ship in the release archive as
`THIRD_PARTY_LICENSES`.

<p align="center">
  <sub>Built by <a href="https://greyforge.tech">Greyforge Labs</a> · <a href="https://sleylang.org">sleylang.org</a> · <a href="https://x.com/SleyLanguage">@SleyLanguage</a></sub>
</p>
