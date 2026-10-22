# Embedding Sley: the `sley-agent` embed API

A host program can compile a Sley program written in the Python dialect of the familiar frontend
(ADR-0055, ADR-0057) in memory, then call its functions with an exact fuel limit. The same code
runs natively and in `wasm32-unknown-unknown`, with identical results.

The API is the module `sley_agent::embed`. It is built with the `familiar` feature, and it does not
need the `native` feature:

```toml
sley-agent = { path = "crates/sley-agent", default-features = false, features = ["familiar"] }
```

## Features

| Feature | Default | Adds |
|---|---|---|
| `familiar` | yes | the familiar frontend (`try --familiar`) and `embed` |
| `native` | yes | workspaces, repositories, drafts, search, the CLI and the `sley-agent` binary |

Without `native`, the library is the pure core: frames, candidate assembly and validation, and
execution. It has no file system, clock or operating-system randomness. That core and `embed` build
for `wasm32-unknown-unknown` without host imports:

```sh
cargo build --release -p sley-agent --no-default-features --features familiar \
  --target wasm32-unknown-unknown
```

A build of the CLI without the familiar frontend is now `--no-default-features --features native`.

## API

```rust
pub struct EmbedOptions { pub seed: u64, pub now_unix_millis: u64 }
pub struct EmbedError { pub code: String, pub message: String, pub line: Option<u32>, pub column: Option<u32> }
pub enum EmbedValue { Int(i64), Bool(bool), Str(String), IntList(Vec<i64>), List(Vec<EmbedValue>) }
pub enum CallResult { Value(EmbedValue), Trap { symbol: String }, OutOfFuel, LimitExceeded { what: String } }
pub struct CallOutcome { pub result: CallResult, pub fuel_used: u64, pub instructions: u64 }

pub fn compile_python(source: &str, options: &EmbedOptions) -> Result<Module, EmbedError>;
pub fn compile_python_with(source: &str, options: &EmbedOptions, ceilings: PolicyResourceCeilings)
    -> Result<Module, EmbedError>;
pub const LARGE_CEILINGS: PolicyResourceCeilings; // `init` ceilings, 65,535 mutations
impl Module {
    pub fn functions(&self) -> Vec<String>;
    pub fn call(&mut self, function: &str, args: &[EmbedValue], fuel_limit: u64)
        -> Result<CallOutcome, EmbedError>;
    pub fn program_digest(&self) -> [u8; 32];
}
```

### Compiling

`compile_python` takes the same path as `sley-agent try --familiar` over a fresh workspace:

1. The familiar frontend parses the text into a structured frame.
2. The frame compiler builds the operations, and the candidate assembler builds a candidate over a
   genesis head held in memory (`genesis::genesis_head`, which matches the policy of `init`).
3. The kernel's in-process candidate validation judges the candidate.

Only a Valid candidate becomes a `Module`. Every function is then lowered and loaded once, and each
call reuses that image.

### Program size

Each entity a program creates is one mutation of its candidate. The `init` grant allows 10,000
mutations, and a 25-line board scan creates about 3,000 entities, so `compile_python` admits about
2.5 KB of such code.

`compile_python_with(source, options, LARGE_CEILINGS)` raises the grant to the kernel's full-v1
operation limit of 65,535. That admits about 16 KB of such code. Beyond it, the candidate is refused
with `SCB_RESOURCE_LIMIT`.

The grant is part of the genesis, so it changes `program_digest`. Its fuel ceiling bounds only
`TestCases`, never `call`.

Refusals carry a stable `AGENT_*` symbol. No new symbol was added:

| Symbol | When |
|---|---|
| `AGENT_FRAME_INVALID` | The frontend or the frame compiler refuses the text. Line and column are given. |
| `AGENT_CANDIDATE_INVALID` | The kernel refuses the candidate. The message has the verdict headline, the locator and the hint, plus the authored line and column when the refusal locates one. |
| `AGENT_NAME_UNKNOWN` | `call` names a function the program does not have. |
| `AGENT_INPUT_INVALID` | The arguments do not fit the function's parameters. |
| `AGENT_EXECUTION_REFUSED` | The VM refuses the call, or the result has no `EmbedValue` form (a record, for example). |

### Calling

`call` runs on the VM with `ExecutionLimits { max_fuel: fuel_limit, ..exec::call_limits() }`. The
outcome maps as follows:

- A returned value is `Value`.
- A trap is `Trap { symbol }`, one of:
  - `unreachable` (including an index out of range);
  - `resource_exhausted`;
  - `adapter_contract_violation`;
  - `internal_invariant`;
  - `trap_<tag>`.
- Running out of fuel is `OutOfFuel`, with `fuel_used` equal to the limit.
- Any other limit is `LimitExceeded { what }`, where `what` is `Instruction`, `ValueUnits`,
  `OutputUnits`, `CallDepth` or `Cancelled`.

Arguments are converted against the declared parameter types. `int` is `i64`. `IntList` is the
cheap form for `list[int]`.

## Determinism

All nondeterminism comes from `EmbedOptions`:

Two other inputs also set the program: the source and the grant (`compile_python_with`).

| Input | Derived from |
|---|---|
| genesis nonce (workspace identity) | `seed` |
| candidate nonce (created entity identities) | `seed` |
| fresh member identities | `seed`, with a counter |
| validation clock | `now_unix_millis` |

Each value derived from the seed is a domain-separated SHA-256 of the seed. The base transaction of
the in-memory genesis is a stand-in identity: a domain-separated SHA-256 of the genesis state root.

Embedded candidates never expire before 2100, so `now_unix_millis` changes no verdict and no digest.

`program_digest` is the proposed state root of the compiled program. For a given source and seed it
is identical everywhere. A different seed gives different identities, and so a different digest. A
host that wants one digest per source fixes the seed.

`fuel_used` and `instructions` are exact VM counts. They are identical for identical calls, natively
and in wasm. `exec::Outcome::micros` is 0 without `native`, because there is no clock.

## Costs to know (measured 2026-10-21, `act(board: list[int] /*2560*/, info, notes) -> int`)

Per-call host overhead with a trivial body (`return 1`, 2 fuel) is about 0.4 ms natively and
0.8 ms in Node wasm. Nearly all of it is the VM's input admission of the 2,650 integers:

| Step | Time |
|---|---|
| building the input values | 45 µs |
| type check | 20 µs |
| canonical-form measurement | 148 µs |
| input hash | 47 µs |

The overhead grows with input size, and fuel does not.

In the Python dialect, an indexed read `xs[i]` with a computed `i` converts `i` from `i64` to `u64`
in a loop over its bits. It costs about 90 fuel plus about 120 fuel per bit of `i`, which is about
1.7k fuel for `board[2559]`. `enumerate` pays the same conversion. A plain `for c in xs` costs about
80 fuel per element. Scan a board with `for c in board` and keep the coordinates in counters.
