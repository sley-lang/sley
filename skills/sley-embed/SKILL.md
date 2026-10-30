---
name: sley-embed
description: Embed Sley in a Rust host or a WebAssembly build with the `sley_agent::embed` API - compile Python-dialect source in memory, call its functions under an exact fuel limit, and get deterministic results and program digests. Use when a Rust program needs to run untrusted or agent-written Sley code, or when a task mentions `compile_python`, `EmbedValue`, `CallResult`, `LARGE_CEILINGS` or `program_digest`.
---

# Embedding Sley

`sley_agent::embed` compiles a program written in Sley's Python dialect
inside the host process and calls its functions with an exact fuel limit.
Compilation takes the same path as `sley-agent try --familiar`: the text is
parsed into a structured frame, built into a candidate over a genesis held in
memory, and judged by the kernel. Only a Valid candidate becomes a `Module`.
There is no file system, clock or OS randomness in this path, so the same
source and options give the same program, results and fuel counts natively
and in `wasm32-unknown-unknown`.

For the dialect itself (types, what is refused, inline tests), see the `sley`
skill or `sley-agent help python`. The full reference is
[docs/EMBEDDING.md](../../docs/EMBEDDING.md). Everything here is pinned to
tag `v2.1.7`.

## Cargo

```toml
[dependencies]
sley-agent = { git = "https://github.com/sley-lang/sley", tag = "v2.1.7", default-features = false, features = ["familiar"] }
```

| Feature | Default | Adds |
|---|---|---|
| `familiar` | yes | the Python-dialect frontend and `embed` |
| `native` | yes | workspaces, repositories, drafts, the CLI and the `sley-agent` binary |

Turn `native` off for embedding. The crate needs Rust 1.93 (edition 2024).
The wasm build of the core:

```sh
cargo build --release -p sley-agent --target wasm32-unknown-unknown
```

run from a project whose dependency line is the one above (inside a Sley
checkout, add `--no-default-features --features familiar`). It needs no host
imports.

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
pub const LARGE_CEILINGS: PolicyResourceCeilings;

impl Module {
    pub fn functions(&self) -> Vec<String>;               // name order
    pub fn call(&mut self, function: &str, args: &[EmbedValue], fuel_limit: u64)
        -> Result<CallOutcome, EmbedError>;
    pub fn program_digest(&self) -> [u8; 32];
}
```

`EmbedError` implements `Display` and `std::error::Error`.

## Example

```rust
use sley_agent::embed::{compile_python, CallResult, EmbedOptions, EmbedValue};

const SOURCE: &str = "\
def score(board: list[int], bonus: int) -> int:
    total = 0
    for cell in board:
        total += cell
    return total + bonus

def pick(xs: list[int], i: int) -> int:
    return xs[i]
";

fn main() {
    let options = EmbedOptions { seed: 7, now_unix_millis: 0 };
    let mut module = compile_python(SOURCE, &options).expect("compiles");
    println!("functions: {:?}", module.functions());

    let args = [EmbedValue::IntList(vec![1, 2, 3]), EmbedValue::Int(10)];
    let out = module.call("score", &args, 1_000_000).expect("call");
    println!("{:?} fuel={} instructions={}", out.result, out.fuel_used, out.instructions);

    let starved = module.call("score", &args, out.fuel_used / 2).expect("call");
    assert_eq!(starved.result, CallResult::OutOfFuel);

    let trap = module.call("pick", &[EmbedValue::IntList(vec![1, 2]), EmbedValue::Int(5)], 1_000_000);
    println!("{:?}", trap.expect("call").result);

    for err in [
        module.call("score", &[EmbedValue::Bool(true), EmbedValue::Int(0)], 1_000).unwrap_err(),
        module.call("nope", &[], 1_000).unwrap_err(),
        compile_python("def f(x: int) -> int:\n    return x / 2\n", &options).unwrap_err(),
    ] {
        println!("{err}");
    }
}
```

Output against `v2.1.7`:

```text
functions: ["pick", "score"]
Value(Int(16)) fuel=120 instructions=23
Trap { symbol: "unreachable" }
AGENT_INPUT_INVALID: arg 0: expected list[i64]
AGENT_NAME_UNKNOWN: no function named `nope`
AGENT_FRAME_INVALID at line 2, column 14: / is float division; Sley ints divide with //
```

## Calls and fuel

`call` runs on the VM with `max_fuel` set to `fuel_limit`. The other limits
are the CLI's `call` ceilings: 100,000,000 instructions, 1,000,000,000 value
units and 16,777,216 output units. Outcomes:

- `Value(v)`: the function returned.
- `Trap { symbol }`: `unreachable` (overflow, index out of range, `//` by
  zero, failed `assert`, `raise`), `resource_exhausted`,
  `adapter_contract_violation`, `internal_invariant`, or `trap_<tag>`.
- `OutOfFuel`: `fuel_used` equals the limit.
- `LimitExceeded { what }`: `Instruction`, `ValueUnits`, `OutputUnits`,
  `CallDepth` or `Cancelled`.

Traps and limits are outcomes (`Ok`); `Err` means the call could not be made.
Each function is lowered and loaded once at compile time and every call
reuses it. `fuel_used` and `instructions` are exact and repeat for identical
calls on every platform. Calling with exactly the fuel a run used returns the
same outcome.

Arguments are checked against the declared parameter types: `Int` for `int`
(and any integer width it fits), `Bool`, `Str`, `IntList` for `list[int]`
(cheaper than `List` of `Int`), `List` for other lists. Records have no
`EmbedValue` form in 2.1.7: a function that takes or returns a record cannot
be called through `embed` (a record result gives `AGENT_EXECUTION_REFUSED`).
Pass structured state as `list[int]` instead. Input admission costs host time
in proportion to argument size, not fuel. A plain `for c in board` loop is
the cheapest way to scan a list; an index computed from arithmetic such as
`board[y * w + x]` pays an integer conversion on each read.

## Determinism

| Input | Derives |
|---|---|
| `seed` | genesis nonce (workspace identity), candidate nonce (entity identities), fresh member identities |
| `now_unix_millis` | the validation clock only; embedded candidates never expire before 2100, so it changes no verdict and no digest |
| the source | the program |
| the grant (`compile_python_with`) | the policy in the genesis |

`program_digest` is the proposed state root of the compiled program. The same
source, seed and grant give the same digest everywhere; another seed or grant
gives another digest. Fix the seed if you want one digest per source.

## Program size and nesting

Each entity a program creates is one mutation. `compile_python` uses the
`init` grant of 10,000 mutations; a function of a few dozen lines can create
thousands of entities. Past the grant, compilation fails with
`AGENT_CANDIDATE_INVALID` and a message naming `max_mutation_count`.
`compile_python_with(source, &options, LARGE_CEILINGS)` raises the grant to
the kernel's limit of 65,535; past that the candidate is refused with
`SCB_RESOURCE_LIMIT`. The grant's fuel ceiling bounds test cases only, never
`call`.

Text nested deeper than 124 levels (parentheses, unary operators, calls,
blocks, `elif` arms, or an operator chain of that depth, such as a sum of more
than 122 terms) is refused with `AGENT_FRAME_INVALID` at its line and column
before any recursive pass runs. Hostile text gets a refusal instead of a stack
overflow. A release build compiles text at that limit on a 2 MiB thread
stack; debug builds use several times more, so run debug hosts or tests that
compile deep text on an 8 MiB thread.

## Refusal symbols

| Symbol | When |
|---|---|
| `AGENT_FRAME_INVALID` | The frontend or frame compiler refuses the text. `line` and `column` are set. |
| `AGENT_CANDIDATE_INVALID` | The kernel refuses the candidate. The message has the verdict, `where:` and `hint:`, plus the authored line and column when the refusal locates one. |
| `AGENT_NAME_UNKNOWN` | `call` names a function the module lacks. |
| `AGENT_INPUT_INVALID` | Wrong argument count, or an argument that does not fit its parameter. |
| `AGENT_EXECUTION_REFUSED` | The VM refuses the call, or the result has no `EmbedValue` form. |
| `AGENT_WORKSPACE_INVALID` | `compile_python_with` got ceilings the policy refuses, such as a zero ceiling. |

Match on `code`; the message text may change between releases.

## What a result means

Embedded calls are advisory execution, as with `sley-agent call`. They are
never admission evidence, never signed and never committed. A Valid module
passed the kernel's static checks; it says nothing about whether the program
does what the host wants. Test the behavior you rely on.
