# Review request REQ-09 rev3 — CREATE native trial admission contract delta

- Supersedes rev2 (sha256 `dd9a02ce...177d1`; verdict FAIL with NTA-P1-1,
  NTA-P2-1, NTA-P2-2, NTA-P3-1 all CLOSED and one new blocker NTA-P1-2;
  transcript `evidence/review/verdicts/native_trial_admission/ariadne-review-b0e52e00-rev2.md`).
  Only NTA-P1-2 is answered; all closed items stand as reviewed.
- Baseline tree: `b0e52e00a2bacfd7a874f0b4a7973a352508cebe` on
  `work/succession-sley20-arm`. Lane: Ariadne (contract delta).
  Scope key: `native_trial_admission`.

## Answer to NTA-P1-2 (stale-probe attempt_id)

- The rev2 packet under-specified the stale probe: it kept the probe's
  session handling implicit while mandating native encoding, which combines
  two independent hazards — (a) a reused (pre-advance) session observes
  `SESSION_ROOT_ADVANCED` at the native precheck (`server.rs:2977-2982`)
  instead of reaching the repository freshness gate, and (b) a reused
  attempt_id takes the journal replay branch (`repository.rs:3266-3267`)
  and returns Committed, inverting the oracle into a false
  `ORACLE_LAST_WRITE_WINS`.
- Rev3 pins the full probe shape under the new profile (changed from the
  current probe shape in exactly these two respects; reviewer assumption 4
  in the rev2 verdict anticipated this re-derivation):
  1. The probe opens a FRESH session bound at the advanced head H1
     (`session.rs:346` binds `bound_root` to the current head), submits the
     same candidate bytes against the OLD parent `pre_tx`, and therefore
     passes the precheck by equality and reaches `commit_native_inner`'s
     freshness gate (`repository.rs:3275-3281`) → `STALE_ROOT`.
  2. The probe mints a DISTINCT attempt_id per probe (fresh random 16 bytes,
     recorded in the run transcript; never the accepted call's attempt_id),
     so `same_bindings` (`native_commit.rs:276-282`) cannot match and the
     replay branch is unreachable for the probe by construction.
- Frozen oracle rule (unchanged code, stated explicitly): acceptance is a
  FAILED response whose decoded symbol is exactly `"STALE_ROOT"`
  (`sley2_live_judge.py:2457-2460`); any non-failed response remains
  `ORACLE_LAST_WRITE_WINS` (`:2450-2451`) — which now correctly classifies
  only a genuine last-write-wins defect, since replay-inversion is excluded
  by (2). The oracle asserts refusal, never replay.
- Bookkeeping correction carried (observation 1): nine PROFILE_ARGS
  consumption sites (1 definition + 9 uses), not eight.

## Re-review trigger (from the rev2 verdict)

Rev3 states stale-probe attempt_id derivation (distinct per probe) with the
frozen stale oracle asserting refusal-not-replay, over the full probe shape
above. Directions 1 (trial-only serve flag) and 3 (additive
NativeTestExecutor adapter) continue unobjected.

- Constraints: read-only review. No file writes. Verdict in reply text only,
  in the REQ verdict format, against scope key `native_trial_admission`.
