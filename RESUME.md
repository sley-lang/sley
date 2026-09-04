# Resume state, 2026-09-04

The Council review round is **running**. The repository is clean and every gate
below was green at the commit named here.

## Where the work is

| Thing | Where |
|---|---|
| Repository | `/home/greyforge/sley2`, branch `main` |
| Review candidate | `/home/greyforge/cache/worktrees/sley2-review-2026-09-04`, detached at `9dc78fe` (four commits behind `main` now; see below) |
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

The round is deliberately still reading `9dc78fe` even though three fixes have
landed on `main` since. That is the point of pinning: a reviewer's findings
stay reproducible from one commit. It also means a reply may raise something
already fixed, so check a landing finding against `main` before acting on it.

## Council reviews: 7 of 69 answered

| Review | Result | P0 | P1 |
|---|---|---:|---:|
| `260-ariadne-contract` | FAIL | 2 | 6 |
| `260-nabu-architecture` | PASS | 0 | 7 |
| `260-vulcan-surface` | FAIL | 1 | 2 |
| `300-ariadne-contract` | FAIL | 1 | 4 |
| `300-nabu-architecture` | FAIL | 1 | 4 |
| `300-vulcan-surface` | FAIL | 0 | 5 |
| `310-ariadne-contract` | FAIL | 2 | 7 |

The first `260-vulcan-surface` reply was not a verdict: it ended mid-object at
2192 characters with no closing brace. Its log is set aside as
`260-vulcan-surface.truncated.log`, it was redispatched, and the second reply is
the one above. **Check every reply for a closing brace before counting it.**

## Findings

**Four P0s closed, two open.**

Closed:

- **S20-260 checked left shift**, fixed before this session.
- **S20-260 map entry order** (`83d571a`). `equal` and `value_hash` read
  ordered-map entry order structurally, but S20-210 does not establish it
  (`TYPE_SYSTEM_V1.md` section 5 reserves the ordering to the SCB codec and
  forbids the checker from reimplementing it or silently sorting). The VM now
  asks the codec at each boundary where a value arrives from outside:
  `VM_EXEC_INPUT_NOT_CANONICAL` (27006) for an execution input,
  `VM_LOWER_IMMEDIATE_MISMATCH` for a constant `constant_ref` or `global_get`
  names. It does not sort and does not tighten S20-210.
- **S20-300 inherited index cache** (`cfdd263`), found independently by Ariadne
  and Nabu. Import now removes the target's `index/` before promoting anything,
  because a cache record is the one entry an incomplete clone carries that
  cannot be proved to belong to the exchange. Contract revision 2.
- **S20-260 cell value units** (`70f4a2e`), found by Vulcan and reproduced
  before fixing. `cell_new` and `cell_set` clone into a table that outlives the
  instruction while only the handle was charged: a 4096-byte payload cost 61
  units against a real 4099. Both now charge what they store, and
  `MAX_EXECUTION_CELLS` caps the table. This moved every observation identity
  touching a cell; all 21 extended vectors were regenerated and the independent
  oracle agrees. Contract revision 10.

Open, both from `310-ariadne-contract` and both contract-text rather than code:

1. `charged_work` is normative in the frozen `SLEYRQR1` record, but the section
   4 charging rule cannot derive the implemented per-class constants, so the
   oracle reproduces the implementation rather than the contract, and S20-320
   already binds these bytes.
2. The class-kind applicability table that ADR-0030 decision 1 and section 9
   require is absent, so classes 16, 17, and 19 silently accept every kind.

**These two are the next thing to work on**, and both are authoring decisions
about `docs/spec/ROOT_BACKED_QUERY_PROFILE_V1.md` rather than defects to patch.

## To resume

1. **The dispatcher is already running.** Restart only if it has stopped:

   ```bash
   cd /home/greyforge/machineresearch/sley-2.0/council-queue
   nohup ./patient_dispatcher.sh >/dev/null 2>&1 &
   ```

   It is restart-safe: it skips any request whose log already holds its
   `_REVIEW_JSON=` key. Roughly five to seven minutes per review.

2. **Answer the two open S20-310 contract P0s** above.

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
