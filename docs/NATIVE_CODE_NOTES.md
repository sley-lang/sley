# Native code: design notes for a future campaign

Status: notes only. No native-code work is scheduled, and nothing here
authorizes any. [ANTI_GOALS.md](ANTI_GOALS.md) keeps native, JIT and AOT
execution out of the release graph unless an authorized campaign declares it
in [`host-boundary.json`](../host-boundary.json) and it passes the anti-goal
gate. These notes record what such a campaign would have to keep, so that it
starts from the VM's existing contracts rather than from a JIT's habits. They
come from issue #86, which studied
[Lust](https://github.com/ArchangelJTW/lust-lang), a Rust scripting language
with a tracing JIT and a method JIT.

## What a campaign must not change

- **Meaning.** Every result, failure edge and trap code is the reference
  interpreter's (`ExecutionMode::Reference`, `sley-agent call --reference`).
  Native code is a derived plan of an admitted image, like the scalar plan
  and the word tier. It is never a second semantics.
- **Accounting.** Fuel, instructions, value units and output units equal the
  reference's exactly, at every cut: every fuel, cancellation, instruction,
  value-unit and output limit, including a stop part-way through a block.
  The word tier is held to this today by differential tests at every cut.
  Lust shows what to avoid here: its JIT counts gas per trace entry and per
  back edge rather than per instruction, so the gas a program uses depends
  on how warm the JIT is. A Sley count may never depend on which tier ran.
- **Admission and identity.** Admission, receipts, state roots, value hashes
  and the image digest are computed exactly as now. Native code is chosen
  after admission and can only be dropped, never consulted, by the kernel.
- **Portability.** `wasm32-unknown-unknown` and every other target keep the
  interpreter tiers and give identical results and counts. Native code is an
  optimization for the targets it supports, never a capability.
- **Kernel safety.** The workspace keeps `unsafe_code = "forbid"`. Code
  generation and executable memory live in a separate crate behind an ADR,
  outside the kernel crates. A native verifier may reject a malformed image
  for memory safety only; it never replaces language-level checking
  (`host-boundary.json`, `verifier_vs_checker`).

## What the VM already provides

- **A reference to diff against.** The reference interpreter runs every
  program, and `--reference` selects it per call. Lust keeps the same shape
  (`LUST_JIT=0` is its reference) and diffs its example programs across both
  modes on each backend change.
- **Prepaid runs.** The scalar plan groups each block's leading steps into
  runs (`Batch` in `crates/sley-vm/src/execute/scalar.rs`) with their
  worst-case fuel and value units. The fuel, cancellation, instruction and
  value-unit checks are each monotone in what was charged before, so when the
  budgets hold a run's worst case, no check inside the run can stop it. The
  run executes without per-step checks and adds its exact totals afterwards.
  The word tier (2.1.5) runs up to 64 whole blocks against one prepaid worst
  case, including direct calls and returns.
- **Exact stops.** When a guard inside a fused run fails (a checked
  operation's error edge, a budget too small for the worst case), the run
  stops with exactly the charges of the blocks already completed, and the
  ordinary loop continues from there with per-step checks. This is the
  precondition for native code with exact accounting: a native run is a
  prepaid run compiled to machine code, and every way out of it is an exit to
  the interpreter at a block boundary that the interpreter has already
  charged.

## A design that keeps those contracts

1. **Unit of compilation: a prepaid run.** Compile only what the scalar plan
   or the word tier already treats as one prepaid run. Its entry check is the
   existing `prepaid` test; nothing inside it charges.
2. **Exits at block boundaries.** Any guard failure (a checked overflow, an
   out-of-range read, a run that would exceed its prepaid case) returns to
   the interpreter at the start of the block it occurred in, with the
   registers of that boundary. The interpreter replays that block step by
   step and charges it exactly. Lust's compiled loops similarly charge at
   back edges and hand control back to the interpreter to raise an error; in
   Sley the charge is decided before entry, so the exit carries no count of
   its own.
3. **Checked arithmetic stays checked.** Every operation that has an error
   edge in SSMC keeps it in native code, with the same width and signedness
   resolved at compile time as in the word tier. No wrapping arithmetic is
   introduced; no float behaviour depends on the host.
4. **Caching.** A native plan is keyed by the image digest and the existing
   bytecode cache-key domain (ADR-0008), and is a pure function of them. A
   missing or refused plan falls back to the interpreter tiers with no
   observable difference.

## How it would be tested

- The differential gates the word tier passes today (result, fuel and
  instruction counts at every cut, call chains to the depth limit), run
  against the native tier.
- A seeded random-program differential fuzzer (issue #86, item 1): generated
  programs run on the reference interpreter, every derived tier and the
  native tier, comparing results and every count at random cuts, with
  shrinking to a replayable seed.
- Mutation testing of the code generator and its exits, as for the VM
  performance releases.
- The `bench/runtime` programs, reporting fuel and instruction counts beside
  times, so a speedup can never come from a counting change.

## What would justify starting

A measured gap on representative workloads that the interpreter tiers cannot
close, against the cost of a separate unsafe crate, a target matrix and a
permanent differential obligation. The decision belongs to a declared
campaign with its own ADR, `host-boundary.json` entry and anti-goal review.
Until then this file is the only artifact.
