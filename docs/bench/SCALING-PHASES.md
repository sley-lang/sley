# Phase-separated scaling probe

This development probe addresses issue #21's request to distinguish bounded
output from work that grows with program size. It makes no constant-time claim.
It uses the source revision recorded with a run, not the published musl release
artifact. No hosted model, native test supervisor, or production workspace is
involved.

Build and run from a clean checkout with an installed Rust toolchain and cached
dependencies:

```sh
cargo build --locked --offline --release -p sley-agent --bin sley-agent --example scaling_phases
python3 scripts/measure_scaling.py --agent target/release/sley-agent --probe target/release/examples/scaling_phases --output /absolute/new/run-directory
```

The parent of the output directory must already exist; the output directory must
not. The runner refuses to overwrite a run. It is bounded to one CPU on Linux,
30 seconds per child command and a 300-second campaign deadline, plus up to two
seconds of cleanup grace per command and record-writing overhead. It checks a
40 GiB free-space reserve before each command; this is not a continuous disk or
memory cap. Each child output stream is capped at 1 MiB.
It makes four fixtures with 1, 10, 100 and 1,000 identity functions, then collects
five fresh-process samples per size in a fixed shuffled order. Both sizes and
sample count can be reduced with `--sizes` and `--samples`. Build time is outside
the measurement. Do not run competing builds during measurement.

Every fixture has one fixed `target(x: i64) -> i64` returning `x`; additional
functions are unused identity functions. Its accepted history has one program
commit. A constant-sized candidate changes only `target` to return literal `7`.
The candidate is validated but never promoted. Fixture creation commits into
new disposable repositories only. Those repositories contain generated genesis
authority: publish the generated authoring frames and numeric results, not the
whole disposable workspace.

The probe measures these phases with `Instant`:

| Field | Includes |
| --- | --- |
| `accepted_head_load` | Read and verify the existing accepted head through `Workspace::read_head` |
| `accepted_program_index` | First `Head::program` clone, sort and object index |
| `name_map_load`, `name_index` | Name-map reads and derived name lookup construction |
| `executor_projection_type_and_function_index` | `Executor::new`: complete-entity projection, type environment and function index together |
| `first_lower_and_verify` | First `Executor::prepare` for the fixed target, including image load and verification |
| `parameter_type_lookup` | Public signature lookup, including the parameter scan |
| `typed_input_decode` | Decode the fixed integer argument through the public value API |
| `first_call_after_explicit_lowering` | First ordinary call after explicit lowering |
| `repeated_call_ns` | 100 ordinary calls using the cached lowered image; each still admits its input |
| `candidate_and_authority_load` | Select the public principal/grant and read the candidate from its store |
| `candidate_validation` | Canonical candidate validation against the already loaded head |
| `proposed_program_index` | Clone, sort and index the validated proposed state |

Every call is checked against the independent identity oracle `target(42) = 42`.
Input clones, oracle checks and memory sampling are outside call timers. Calls
use ordinary `Executor::run`, not the pre-admitted `PreparedCall` optimization.
The example includes the CLI's existing allocator module without changing it.

The runner also records end-to-end wall time for a fresh empty probe process,
the full probe, a focused CLI view, and a CLI call. The empty baseline includes
process launch, startup, minimal argument handling, output, and shutdown; it is
**not** an isolated loader measurement and is not subtracted from other timings.
Wall measurements include Python process launch and pipe collection overhead.

Each sample starts a fresh process, with no persistent executor or lowered-image
cache. The filesystem cache is warm from fixture creation; the runner does not
drop OS caches or claim cold-disk performance. Phase measurements within one
process are ordered and can reuse allocator and filesystem state. Memory reads
and reporting also affect that process; phase times are not expected to sum to
its end-to-end wall time. Linux `VmRSS` and `VmHWM` snapshots report total process
resident and high-water memory, not per-phase allocations or exclusive memory.
After all those timings and snapshots, the probe checks function counts, the
unchanged accepted head and non-target objects, preserved target identity, and
the proposed result `target(42) = 7`. These additional checks are timed separately
and included in total probe process time. The CLI also does its own cloning and
selection, so probe phases are not a direct decomposition of CLI wall time.

`samples.json` retains every sample; `commands.json` retains setup and command
outcomes, and `fixtures.json` hashes the deterministic frames. Binary and runner
hashes are in the manifest. Record the source commit, modified-file hashes,
toolchain, build profile, CPU, shared-host load and any failed attempts alongside
the run. Report medians with observed ranges, not confidence intervals from this
small sample. A single host and synthetic unrelated functions do not establish
scaling for large function bodies, call graphs, type graphs, test sets, long
histories, arbitrary edits, or other machines.

The first measured run is documented in
[SCALING-2026-10-15.md](SCALING-2026-10-15.md), with portable raw samples.
