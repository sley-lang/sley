# S20-700 Semantic-Checker Persistent Slice

Status: scoped persistent landed-surface slice; **full S20-700 remains incomplete**

This slice adds two libFuzzer targets at existing public typed boundaries:

- `type_checker` constructs bounded `TypeDefinition` and `TypeExpr` values and
  repeats the S20-210 environment, shape, trait, and instantiation judgments;
- `ssmc_graph_cfg_checker` starts from four accepted function-graph templates,
  applies up to eight mutations from 33 graph and CFG mutation classes, and
  repeats the public S20-220 judgment.

Both targets cap input at 4,096 bytes. The type generator also has a global
512-node construction budget. The graph target asserts every unmutated template
remains accepted. Both targets assert repeated judgments are identical.

These byte-to-structure mappings are fuzz-only constructors. They are not a
canonical SSMC decoder, do not expose the crate-private partial mutation codec,
and do not define serialized graph, type, CFG, or mutation authority. The graph
target covers the current public S20-220 graph-inventory and CFG boundary; it
does not claim a future complete SSMC object decoder.

The deterministic runtime corpora contain 385 type-checker seeds and 400
graph/CFG seeds. Corpus, binaries, artifacts, and command evidence remain under
ignored `evidence/runtime/s20-700-semantic-checkers-libfuzzer/` paths.

## Closed harness finding

`S20-700-HARNESS-001` is the minimized one-byte input `c2`. The first smoke run
showed that the original fuzz-only type generator could expand a cyclic byte
stream exponentially before its depth limit and hit libFuzzer's memory limit.
This was not a production checker defect. The generator now enforces the global
512-node budget, the minimized input is retained in
`fuzz/regressions/S20_700_HARNESS_001.json`, corpus generation consumes that
fixture, and the repeated smoke run passes.

Independent Vulcan review remains deferred because the local Forge OAuth
session returns 401. Mutation candidates, merge, and the full finding register
remain required.

Focused validation:

```text
cargo +nightly-2026-02-27 clippy --manifest-path fuzz/Cargo.toml --bin type_checker --bin ssmc_graph_cfg_checker -- -D warnings
python3 scripts/check_semantic_checkers_persistent_fuzz_slice.py
make semantic-checkers-persistent-fuzz-smoke
python3 scripts/run_semantic_checkers_persistent_fuzz.py --manual --target type-checker
python3 scripts/run_semantic_checkers_persistent_fuzz.py --manual --target graph-cfg
```

## Repair round 7 (REQ-06 fuzz repair wave)

The REQ-06 REVISE findings against this slice's harness are repaired in
the runner: `--locked` builds, workspace-wide owner-lib coverage via
`-Zhost-config` + target rustflags with trace-compares/pc-table (the old
bin-only flag left zero `sancov` symbols in owner rlibs; the gate now
fails closed on zero family-wide symbols and zero symbols in the slice's
owner rlib), an on-disk coverage floor (corpus files plus 256 guaranteed
mutations, replacing seed-count enforcement, which decayed as libFuzzer
added inputs), persistent corpus directories with stale-seed sync, crash
minimization via `-minimize_crash=1` with exact artifacts (round-7c fixed
the `-merge=1` primitive, which merges corpora and cannot minimize a
crasher), executed/inline-counter-coverage/crash evidence gates, a
dedicated build timeout, and append-only artifacts. The slice proved
locally PASS with executed >= floor, inline 8-bit counters observed,
zero crash artifacts, and nonzero owner-rlib `sancov` counts; the durable
record is `machine-summary.json` `last_local_proof` (runtime
`evidence.json` files are gitignored by design). The decoder is safe
Rust, so no ASan is instrumented; the oracle and seed neighbourhood are
unchanged (bounded smoke, not a probe). The pinned qualification
toolchain is unchanged (`clang-18`, pinned libfuzzer path,
`nightly-2026-02-27`); the local proof ran under documented
`SLEY_FUZZ_CC` / `SLEY_FUZZ_LIBFUZZER_A` overrides, and the pinned
qualification default itself has no recorded proof on this host (the
evidence `toolchain_versions` field captures exactly what ran).
Re-review of the slice's Vulcan verdict is queued, not assumed.

## Rounds 7c-7m (REQ-06 re-review wave)

Crash minimization uses `-minimize_crash=1` with exact artifacts (the
round-7 `-merge=1` primitive could not minimize a crasher); the coverage
floor measures on-disk corpus files plus 256 mutations; coverage gates
strictly on inline counters with monotonic `ft` (no silent fallback);
the owner gate counts the rlibs cargo linked (fingerprint-authoritative,
`rlib_linkage` recorded, fail-closed with no mtime fallback); warnings are
captured from the full streams against an explicit allowlist; builds
refuse ambient `RUSTFLAGS`; prior crashers re-execute every smoke
(crash-to-regression); per-input `-timeout=30` and `-rss_limit_mb=2048`
bound hangs; libFuzzer seeds are recorded; build provenance
(`build_locked`, `sancov_scope`) derives from the executed argv; the
fuzz profile enables overflow checks and debug assertions. Manual
campaigns remain operator exploration (no floor/limits parity by
design). Slice oracles were strengthened per verdict (engine-invariant
asserts, must-reject refusals, narrowed Err arms, constructed-valid
import/decode lanes); the pinned clang-18 default has no recorded proof
on this host.

## Rounds 7k-7m (second re-review wave)

The linked-rlib replay carries the build environment (an env-less replay
rebuilt and relinked the binary after the recorded build); the mtime
fallback is deleted, so a failed cargo query fails the gate instead of
passing on unknown provenance. Crash gating distinguishes new crashes
from retested priors (fixed priors pass with a recorded retest). The
minimize step skips clean-retested artifacts, bounds internal steps, and
keeps partial exact artifacts; durations compute last. Slice oracles
gained engine-invariant asserts, must-reject refusals, narrowed Err
arms, constructed-valid lanes, and a server-fixture validity gate with
a unit-level negotiation self-check; filed regressions replay as corpus
seeds. See the wave's decision packets for elevated owner items.

## Target-closure wave (semantic-Err narrowing, operator-authorized redesign)

Over-broad acceptance took the form accept-any-deterministic-code: both
targets asserted only judgment equality, so any deterministic wrong code
passed. The graph target now threads each applied mutation class out of
`apply_mutation` and asserts the failure belongs to that class's
narrowest contractually correct set (single mutation: the exact class
set; several: their union, with the determinism assert pinning which
class stably wins): inventory/owner/ordinal/duplication arms bind the
`GRAPH_*` group plus `CFG_ENTRY_INVALID` where entry resolution is at
stake; terminator/target/argument arms bind the `CFG_*` target group plus
`CFG_RETURN_TYPE`/`CFG_VALUE_UNRESOLVED` where reachable; value-use arms
bind `CFG_VALUE_UNRESOLVED`/`CFG_RESULT_INDEX`/`CFG_USE_BEFORE_DEFINITION`;
duplication arms bind exactly `GRAPH_DUPLICATE_ENTITY`; result-type
mutations bind the exact `TYPE_*` set. The type target adds
cross-judgment coherence: orderable/hashable/persistable `Ok` requires
the matching trait flag on a successful `traits` judgment; a checked
closed type must carry traits; and checked arguments of matching length
substituted into a checked type must never fail `TYPE_ARGUMENT_ARITY`.
Sets widen only with a documented contract reason, never by fiat; a
wrong-code control run (deliberately corrupted expectation) must fail.
No change to `TypeErrorCode`/`CfgErrorCode` sets, check order, or
release-profile behavior. Re-review queued as REQ-08 item 7; proofs bound
to older source states are invalid.

## Second repair round (review-derived sets + durable control record)

The fresh review (REQ-08 item 7, REVISE) derived three reachable correct
codes from checker precedence that the first-round sets omitted, and the
oracle confirmed each by failing on correct engine behaviour before the
widening: arm 27 reports `TYPE_PARAMETER_OUT_OF_SCOPE` (result-type
push of a free parameter; `CFG_RESULT_INDEX` removed — pushing cannot
produce it); terminator arms report `CFG_RESULT_INDEX` (result_index % 4
against single-result template operations) and `CFG_DOMINANCE` (Block-role
parameter used from a reachable non-owner block); arm 0 narrows to
`GRAPH_DUPLICATE_ENTITY`/`CFG_ENTRY_INVALID` (entry resolution precedes
the inventories). Three single-mutation inputs pin the newly covered
paths as permanent corpus seeds (arm 27 `03 01 1b 00 06`, terminator
index `03 01 13 00 01 00 02 02`, terminator dominance
`01 01 15 00 00 00 02 00 04 01 00 00 01 00 04 01 00 00 01`);
graph-cfg seeds 396 -> 399.

Wrong-code control (durable record): with arm 28/29/30's expectation
corrupted to `GRAPH_INVENTORY_MISMATCH` (one-line sed, uncommitted
scratch), the smoke FAILs on the committed class-28 control seed with
`escaped with unexpected failure class GRAPH_DUPLICATE_ENTITY`
(expected one of `["GRAPH_INVENTORY_MISMATCH"]`); reverted, the lane
returns to PASS. The oracle discriminates rather than accepting any
registered error. The control's crash artifact was pruned from
`graph-cfg-artifacts/` during repair (runner is append-only; pruning is
an explicit owner step, recorded here). The closed-traits coherence
assert is sound because `check_type` is `check_type_inner` plus
`check_map_keys` over the same fields at the same depth accounting (any
DepthLimit `traits_inner` could reach is returned first by `check`).

## Third repair round (union closure + dead-entry correction)

The qualifying re-review (fixed lane at 16a095e, REVISE) verified the
second-round fix complete at the single-mutation level, then falsified
the lane one level up: `CFG_UNREACHABLE_VALUE` is a correct
deterministic engine code reachable inside the harness envelope by three
mutations (classes [4, 18, 20] on template 1: entry rerouted to block
13, block 12 flipped to ExplicitlyUnreachable, block 12 given a Branch
using Parameter(14) owned by block 13 — the terminator loop still visits
unreachable block 12, and `resolve_value` reports UnreachableValue at
`cfg.rs:504-506` instead of Dominance), yet it appeared in no arm's set,
so the union assert panicked on correct engine behaviour. The session
owner confirmed the counterexample with a direct binary run (byte-exact
panic signature) before repairing.

Fix: `CFG_UNREACHABLE_VALUE` is bound with a contract reason on arm 18
and the value-using terminator arms 19/20/21/22/32 (the only arms that
can place a value-use in an unreachable use-block in this envelope —
verified: single-block templates cannot unreach their entry, multi-block
templates hold no operations, and no arm creates operations, so arm 26
cannot participate); the 15-byte input pins the path as permanent corpus
seed `seed-0007` (graph-cfg seeds 399 -> 400). Dead entries with false
contract reasons are removed, each re-derived from the engine (arm 1
`GRAPH_INVENTORY_MISMATCH`: :673-679 precede the count check; arms 2/6/31
`GRAPH_INVENTORY_MISMATCH`: reversal preserves the multiset, and arms 6/31
bind the empty set — block/operation table order is validated nowhere,
so they cannot fail; arm 11 `GRAPH_INVENTORY_MISMATCH`: id/ordinal kept,
owner check first; arm 13 `CFG_VALUE_UNRESOLVED`: type change cannot
unresolve an id; arm 15 `GRAPH_INVENTORY_MISMATCH`: back-pointer outside
the inventoried sets; arm 24 `GRAPH_INVENTORY_MISMATCH` +
`GRAPH_UNRESOLVED_REFERENCE`: the declaring block still names the op, so
the owner check fires first; arm 22 `CFG_DOMINANCE`: a Trap at the entry
orphans the target and reachability precedes terminators, a Trap in the
non-entry block uses self-owned values). Over-wide-but-sound entries are
narrowed where derived (arms 9/14/23); arms 3/13 keep the full
`TYPE_CODES` deliberately — the round-2 arm-27 narrowing reasoning does
not transfer without verifying `small_type` case-7 (`AdapterHandle`)
semantics through `check_type`, so wideness stays until derived.

Also recorded (fourth set change, mid-repair): narrowing arm 0 exposed a
harness false-positive — template-2 input pushing block-param 25 onto
`function.parameters` reports `GRAPH_OWNER_MISMATCH` (retained artifact
`crash-47e105f9…`, five mutations [1, 8, 0, 0, 0]), so arm 1 gains
`GRAPH_OWNER_MISMATCH`. Artifact dispositions: `crash-47e105f9…` is a
documented harness-oracle correction probe (retests clean, permanent
retest seed); `minimized-615e37…` is its failed-minimization stub
("did not crash", permanent retest seed). The slice checker now pins the
four review-derived codes and both regression-seed blocks, so a silent
revert of any repair round fails the contract. Fresh PASS proof 937/937
on both targets (override lane); fix awaits re-review.
