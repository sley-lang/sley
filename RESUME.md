# Resume state, 2026-09-04

The Council review round is **running**. The repository is clean and every gate
below was green at the commit named here.

## Where the work is

| Thing | Where |
|---|---|
| Repository | `/home/greyforge/sley2`, branch `main` |
| Review candidate | `/home/greyforge/cache/worktrees/sley2-review-2026-09-04`, detached at `9dc78fe` |
| Council review queue | `/home/greyforge/machineresearch/sley-2.0/council-queue/` (durable, and gitignored there) |
| Retained verdicts | `machineresearch/sley-2.0/reviews/` plus `reviews/verdicts.json` |
| Checkpoint narrative | `machineresearch/sley-2.0/COUNCIL_REVIEW_CHECKPOINT_2026-09-04.md` |

## Reviews run against an immutable candidate now

The first dispatches read the live checkout while it was being edited. The
Greyforge Git Guard recorded `Dirty repo after openclaw` on `260-vulcan-surface`
and all three `300` logs, which is the review mode's "immutable candidate"
requirement being violated: a reviewer reading a moving tree can produce
findings that match no commit.

Reviews now run inside a detached worktree pinned at `9dc78fe`. Every request
names that path, and `patient_dispatcher.sh` changes into it. Work on `main`
no longer touches what a reviewer sees. **When the round finishes, repin the
worktree (or make a new one) before starting another round, or reviewers will
be reading an old candidate.**

## Council reviews: 5 of 69 answered

| Review | Result | P0 | P1 |
|---|---|---:|---:|
| `260-ariadne-contract` | FAIL | 2 | 6 |
| `260-nabu-architecture` | PASS | 0 | 7 |
| `260-vulcan-surface` | truncated, requeued | - | - |
| `300-ariadne-contract` | FAIL | 1 | 4 |
| `300-nabu-architecture` | FAIL | 1 | 4 |
| `300-vulcan-surface` | FAIL | 0 | 5 |

`260-vulcan-surface` is not a verdict: the reply ended mid-object at 2192
characters with no closing brace. It is recorded as `TRUNCATED_REDISPATCH`, its
log is set aside as `260-vulcan-surface.truncated.log`, and it is back in the
queue. **Check every future reply for a closing brace before counting it.**

## Findings

**Both S20-260 P0s are closed.** The checked left shift was fixed earlier. The
map entry order is fixed at `83d571a`: `equal` and `value_hash` read ordered-map
entry order structurally, but S20-210 does not establish that order
(`TYPE_SYSTEM_V1.md` section 5 reserves it to the SCB codec and forbids the
checker from reimplementing it or silently sorting). The VM now asks the codec
at each boundary where a value arrives from outside and refuses one with no
canonical form: `VM_EXEC_INPUT_NOT_CANONICAL` (27006) for an execution input,
`VM_LOWER_IMMEDIATE_MISMATCH` for a constant `constant_ref` or `global_get`
names. It does not sort, and it does not tighten S20-210.

**One P0 is open**, and two reviewers found it independently: exchange import
allowlists a pre-existing `index/v1` cache into the target
(`crates/sley-repo/src/exchange.rs`, allowlist near line 71-79, test near 2411),
so a cloned repository adopts a cache it never wrote, and the S20-300 contract
section 5 bound "same local filesystem authority as objects, receipts, and refs"
is false on that path. Both reviewers propose the same remedy: import must clear
`index/` or refuse the target. **This is the next thing to fix.**

## To resume

1. **The dispatcher is already running.** Restart only if it has stopped:

   ```bash
   cd /home/greyforge/machineresearch/sley-2.0/council-queue
   nohup ./patient_dispatcher.sh >/dev/null 2>&1 &
   ```

   It is restart-safe: it skips any request whose log already holds its
   `_REVIEW_JSON=` key. Roughly five to seven minutes per review.

2. **Fix the open S20-300 P0** (exchange import and the `index/v1` cache).

3. **Triage each landing verdict** into the S20-740 finding register by
   recording the disposition in `machineresearch/sley-2.0/machine-summary.json`
   and rebuilding, and keep `reviews/verdicts.json` current. Take reviewer
   counts from the emitted JSON, not the prose.

## Gates

Council model access is open. Five stand, all outside the integrator: the
narrowed schema-epoch decision, succession trials (model access plus spend
authorization), the root license text, second-host attestation, and the release
decision.

## Validation at this commit

`make quick`, `make lint`, Tier 2 (`core`, `conformance`, `adversarial`,
`fuzz-smoke`), `make vm-persistent-fuzz-smoke`, and the full `cargo test
--workspace` all passed. One combined Tier 2 invocation exited 2 once and did
not reproduce across three later runs, individually or combined; every
individual target passes.

Two clippy errors exist in `fuzz/targets/root_query_engine.rs` and
`context_capsule_builder.rs` (`manual_is_multiple_of`). They are pre-existing:
`make lint` runs `cargo clippy --workspace`, which does not include the separate
`fuzz` crate, so that crate has never been linted under `-D warnings`.
