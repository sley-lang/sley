# ADR-0052: the sley-agent binary's allocator, the workspace's one unsafe exception

Status: accepted for Sley 2.0.2 (2026-09-26, operator decision). It extends
ADR-0051 decision 7.

Date: 2026-09-26

## Context

`call --batch` runs one function over many inputs, and a batch should cost
little more than the work its inputs need.

The release archive's binaries are static musl builds
(`scripts/build_release_candidate.py`). musl's allocator hands freed memory
back to the kernel eagerly. A batch allocates and frees the same sizes for
every input, so it pays for page faults and system time every time.

Measured on the release configuration after `VerifiedImage` (ADR-0051
decision 7), for 1,000-input batches of fifteen small programs:

| Build | CPU per batch | Page faults (one call, a large candidate) |
|---|---|---|
| musl | 34-55 ms | about 1,900 |
| Same code on glibc | 18-30 ms | about 700 |

On musl, about half of the batch's CPU is system time.

The workspace lint is `unsafe_code = "forbid"`, and `docs/ANTI_GOALS.md`
admits an exception only when an ADR and review isolate it. An allocator is
unsafe by definition: it implements `GlobalAlloc`.

## Alternatives considered

- **A glibc release build of `sley-agent`.** A static glibc links the build
  host's `libc.a`, so builds on different hosts differ. That would break the
  release's multi-host reproducibility (`MULTI_HOST_REPRODUCIBLE`).
- **A third-party allocator crate (mimalloc, jemalloc).** These add C code and
  supply-chain surface to the archive. 2.0.2 states that `sley-agent` adds no
  third-party crate.
- **Fewer allocations in the kernel crates.** The churn is spread across
  repository loading, candidate validation, the value codec and the VM. That
  is broad kernel change for a tool-layer cost.
- **No change.** Batches keep paying musl's page faults and system time.

## Decision

1. **One module of the binary target.** `crates/sley-agent/src/allocator.rs`
   defines `SizeClassCache`, a `GlobalAlloc` over `std::alloc::System`, as
   a module of the `sley-agent` binary target (`src/main.rs`,
   `mod allocator;`).
   - It is not part of the library, and no crate can depend on a binary.
   - The library's root forbids unsafe code (`#![forbid(unsafe_code)]`), so
     the compiler still rejects unsafe code anywhere in the workbench
     library.
   - The workbench integration tests include the module by path and install
     it, so the whole suite runs on it.
2. **What it does.**
   - It caches freed blocks of up to 64 KiB, with alignment up to 16, per
     thread and per power-of-two size class.
   - Every cached block was allocated from `System` with its class layout.
     Larger or more strictly aligned requests go straight to `System`, and
     so do zeroed ones that miss the cache (calloc keeps its lazy zeroing).
   - A thread keeps at most 2 MiB and at most 1,024 blocks per class, about
     26 MiB in the worst case. A block freed beyond that goes back to
     `System`.
   - When `System` fails, the thread's cached blocks go back to it and the
     request is retried once.
   - A thread's cached blocks are not returned when the thread exits; they
     stay within those bounds. The binary runs on one thread.
3. **Provenance.** The per-class lists hold addresses in constant,
   destructor-free thread-locals. Nothing is written into a freed block.
   - Each cached block's provenance is exposed when `System` first
     allocates it.
   - A block reused from the cache, returned by the same-class `realloc`
     path, or released from the cache to `System` is rebuilt from that
     exposed provenance.
   - A block therefore carries rights over its whole extent, whichever
     narrower pointer (a `Box<u8>`, say) freed it. This holds under both
     Stacked and Tree Borrows. The review's Miri runs showed that an earlier
     design, with intrusive links written through the freeing pointer,
     violated Stacked Borrows.
4. **The lint stays as strict as it can.**
   - The workspace keeps `unsafe_code = "forbid"`.
   - `sley-agent` copies the workspace lint table with
     `unsafe_code = "deny"`, because a `forbid` would reject the module's
     `#![allow(unsafe_code)]`.
   - The library then restores `forbid` at its root.
   - Every `unsafe` block states its safety argument.
5. **The exception is checked mechanically, on every `make quick` and
   `make release-candidate-verify`.** The command is
   `scripts/build_anti_goal_conformance.py --check-unsafe`. The anti-goal
   report carries the same row.
   - The scan masks comments and literals (`rust_source_regions`). It flags
     the `unsafe` keyword in any form, and any `allow`, `expect` or `warn`
     that names `unsafe_code`.
   - It exempts only `allocator.rs`, and only while all of these hold:
     - the module carries exactly one relaxation, its
       `#![allow(unsafe_code)]`, and implements `GlobalAlloc` for
       `SizeClassCache`;
     - it contains no out-of-line module, `#[path]`, `include!`,
       `macro_rules!`, exported or placed symbol, `extern`, inline assembly,
       mutable static, transmute, `from_raw_parts_mut` or allocator
       installation;
     - the library forbids unsafe code and does not include the module;
     - `main.rs` declares the module and uses no unsafe code;
     - only `main.rs` and the workbench tests install a global allocator,
       and only the tests include the module by path;
     - the parsed `sley-agent` lint table equals the workspace's, except
       `unsafe_code = "deny"`;
     - every other member inherits the workspace lints;
     - `sley-agent` has no build script.
   - Any other unsafe source, or any broken condition, fails the anti-goal
     "unsafe code hidden in kernel".
6. **No value changes.** The allocator decides where bytes live, never what
   the workbench computes. Batch outputs, fuel, instruction counts, TestCase
   reports and refusals are identical with and without it.

## Review

An adversarial review ran three lenses (soundness against the `GlobalAlloc`
contract, runtime behavior and resource bounds, policy isolation), with an
independent attempt to refute each finding. The earlier design had these
findings, all addressed above:

- **The exception was the whole package (major).** The allocator was a
  public module of the library, the relaxation covered every target, and
  the line-anchored scan missed expression-position `unsafe`, item-level
  allows and `unsafe` attributes. Fixed by decisions 1, 4 and 5.
- **The module's shape check was incomplete.** It missed submodules,
  `include!` and unsafe linkage attributes. Fixed by decision 5.
- **Retention was unbounded.** It grew without bound across threads,
  nothing was given back on allocation failure, and the retention claims
  were wrong. Fixed by the caps and the drain in decision 2.
- **No routine gate ran the check.** Fixed by decision 5.
- **Missing overrides.** `alloc_zeroed` bypassed calloc; fixed by
  decision 2. The Stacked Borrows provenance violation was judged
  model-dependent, since Tree Borrows accepts it. It is fixed anyway by
  decision 3.

The review found these sound, and they stay:

- layout pairing and the `realloc` paths;
- alignment;
- no reentrancy (thread-local access compiles to plain loads and stores);
- no reachable panic.

It also checked the binary on real batches: outputs were byte-identical, and
100,000-input batches used 1.15 s of CPU against 2.2 s.

## Consequences

- Measured on the release configuration, for the same fifteen programs on
  a lightly loaded host, a 1,000-input batch takes 15-34 ms of CPU, against
  34-55 ms without the cache.
  - Startup is about 14 ms of CPU, the same as a glibc build. It is
    dominated by the kernel's verification of the accepted head and of the
    candidate.
- Peak RSS rises by about 1.5 MB on 20,000- to 100,000-input batches.
- The module's unit tests cover:
  - class mapping and depth bounds;
  - reuse within a class across a size change;
  - release beyond the depth;
  - zeroed reuse and zeroed pass-through;
  - `realloc` across classes and past the cache, contents preserved;
  - pass-through for large and over-aligned requests;
  - distinct live blocks;
  - blocks moving between threads.
- A future release could remove the exception if the kernel crates stop
  churning allocations, or if the archive moves to a toolchain whose
  default allocator keeps freed memory.
