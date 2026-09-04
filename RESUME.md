# Resume state, 2026-09-04

Paused deliberately at the operator's request. Nothing is running in the
background; the repository is clean; every gate below was green at the last
commit.

## Where the work is

| Thing | Where |
|---|---|
| Repository | `/home/greyforge/sley2`, clean, HEAD is the commit below |
| Council review queue | `/home/greyforge/machineresearch/sley-2.0/council-queue/` (durable; **moved out of the session scratchpad**, which `/tmp` may clear) |
| Retained verdicts | `machineresearch/sley-2.0/reviews/` plus `reviews/verdicts.json` |
| Checkpoint narrative | `machineresearch/sley-2.0/COUNCIL_REVIEW_CHECKPOINT_2026-09-04.md` |

## What changed outside the repository

`~/.openclaw/openclaw.json`: `plugins.allow` gained `anthropic`, and
`plugins.entries.anthropic.enabled` is `true`. That opened the Council lane.
Backup of the pre-change file is in the old session scratchpad; the change is
two keys and is described in the checkpoint document.

## Council reviews: 2 of 69 answered

- `260-ariadne-contract`: **FAIL**, 2 P0, 6 P1.
- `260-nabu-architecture`: **PASS**, 0 P0, 7 P1. "Profile separation is sound;
  defects are contract completeness and vector coverage, not structure."

P0-2 (signed `int_shl_checked` inverted at the top of every width) is **fixed**
and pinned by `e2_left_shift_is_exact_at_every_width_boundary`; restoring the
old multiplier fails that test.

P0-1 is **open and reproduced only by reading**: `check_map_constant`
(`crates/sley-check/src/lib.rs:829`) imposes no key order, and the VM's
equality is `left == right` on `ConstValue`
(`crates/sley-vm/src/extended.rs`, the `(Opcode::Equal, [left, right])` arm),
which is order-sensitive for `ConstData::Map`. Maps built by `map_new` are
sorted; maps arriving as execution inputs or through `constant_ref` and
`global_get` are not. So two semantically equal maps can compare unequal and
hash differently, which makes the observation identity representation
dependent. Not yet demonstrated by a test.

## To resume

1. **Restart the reviews** (they are restart-safe: the dispatcher skips any
   request whose log already holds its `_REVIEW_JSON=` key):

   ```bash
   cd /home/greyforge/machineresearch/sley-2.0/council-queue
   nohup ./patient_dispatcher.sh >/dev/null 2>&1 &
   ```

   It probes `openai/gpt-5.6-sol` then `claude-cli/claude-opus-5`; the second
   answers. Roughly seven minutes per review, so about eight hours for the
   remaining 67. `260-vulcan-surface` was interrupted mid-flight and will be
   redone.

2. **Finish P0-1**: write the failing test first (two maps, same entries,
   different order: `equal` should be true and the value hashes should match),
   then decide where normalization belongs. Note that making S20-210 reject
   unsorted map constants would change which artifacts are accepted, which
   `EPOCH_MIGRATION_POLICY_V1.md` section 1 makes an epoch-forcing change; a
   VM-side normalization at its own boundary is the profile-shaped option.

3. **Triage each landing verdict** into the S20-740 finding register, and keep
   `machineresearch/sley-2.0/reviews/verdicts.json` current.

## Gates

Council model access is **open**. Five stand, all outside the integrator:
the narrowed schema-epoch decision, succession trials (model access plus spend
authorization), the root license text, second-host attestation, and the
release decision.

## Validation at the pause

`make quick`, `make lint`, and Tier 2 (`core`, `conformance`, `adversarial`,
`fuzz-smoke`) all passed at HEAD. `make persistent-fuzz-all` last ran green
over all nineteen slices in 3m40s with no artifact.
