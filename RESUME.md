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

## Council reviews: round complete

All 69 dispatched; 68 answered (67 FAIL, 1 PASS) and one truncated and
requeued. `machineresearch/sley-2.0/reviews/` holds every retained log,
`verdicts.json` the counts, and `p0-worklist.json` the derived P0 list.

The round raised **106 P0 entries** across 23 packages. Two replies arrived
truncated mid-object; **check every reply ends in `}` before counting it**.


## Findings: 17 of 106 P0 entries closed

Closed, each reproduced before fixing:

- **S20-260** (3 entries): checked left shift; ordered-map entry order
  (`83d571a`); cell value units (`70f4a2e`).
- **S20-300** (2 entries, found independently by two reviewers): an exchange
  import adopting an index cache it never wrote (`cfdd263`).
- **S20-620** (12 entries, 7 distinct defects, the round's largest cluster):
  the handle that was not a capability boundary (`28ec1fc`); `context_bytes`
  undercounting and the arm grading itself (`48b71e9`); the swallowable guard
  (`a0d8a6f`); the unfrozen affordance allowlist and protocol failures read as
  candidate verdicts (`20e8bf4`); the shared execution controls narrowed per
  arm (`1e5e77b`).

**89 entries remain open.** Largest clusters: 400 (8), 320 (7), 520 (7),
630 (7), 330 (6), then 710full/720/730/750 (5 each).

Two patterns account for most of what has been closed, and are worth carrying
into the rest:

1. **A contract asserted an invariant nothing executed.** The map order, the
   cell units, the handle boundary and the affordance list were all like this.
   When a reviewer cites a clause, check whether anything runs it.
2. **A measurement favoured the thing it measured.** `context_bytes`,
   `invalid_candidates` and the oracle override all read low for the arm under
   test. Ask which side an error falls on.


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
