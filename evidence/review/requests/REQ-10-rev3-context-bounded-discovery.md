# Review request REQ-10 rev3 — CONTEXT bounded discovery contract delta

- Supersedes rev2 (sha256 `a7aec1e6...f123`; verdict REVISE narrowed to item
  1: option-B availability, option-A head-divergence, option-A cost class;
  transcript `evidence/review/verdicts/context_bounded_discovery/nabu_architecture_review-rev2-b0e52e00.md`).
  Rev2 corrections (class-tag withdrawal, trace-error ownership, preimage
  anchors, P2 ownership gate) stand as reviewed and are not relitigated.
- Baseline tree: `b0e52e00a2bacfd7a874f0b4a7973a352508cebe` on
  `work/succession-sley20-arm`. Lane: Nabu (architecture).
  Section `context_bounded_discovery`, field `nabu_architecture_review`.

## Answers (item 1 only; no new method; no third semantics)

- Option B WITHDRAWN. `session.open` is agent-denied (`runner.py:138`); a
  snapshot identity in its response discloses to the runner, and any relay to
  the agent is harness-relayed task input, not an interface route. It is
  classified as such and rejected: relaying collapses the packet's own
  "authorized inputs only" criterion into the witness-manifest pattern the
  delta exists to replace. Only option A remains.
- Option A, head-divergence: field 9 is populated ONLY for the accepted head
  — the same revision `query_root` pins (`server.rs:2665`,
  `repository.rs:881-905`). For any other requested revision the field is
  ABSENT (omission, not a new refusal symbol): disclosure and discovery agree
  by construction, and the `QUERY_SNAPSHOT_MISMATCH` equality then compares
  two values derived from one revision. Mismatch remains possible only if the
  head advances between disclosure and discovery; that case keeps its existing
  symbol and session-staleness handling, unchanged by this delta.
- Option A, cost class and placement: field 9 reads the ALREADY-MATERIALIZED
  head snapshot identity only. If the head snapshot is not materialized, the
  field is absent (builds stay on the existing snapshot path through
  `complete_root_snapshot`/`judge_complete_root`, never behind the metadata
  route). `revision.read` therefore stays O(1) metadata with
  `returned_entities = 1` truthful — no build cost hides behind it, and the
  deferred Vulcan accounting audit sees exactly what runs. Placement: an
  index-domain value disclosed through the txn-domain route; the cross-domain
  note is carried explicitly into the deferred Ariadne profile-text review
  rather than resolved here.
- P2 ownership gate retained: no third reverse-impact path; the
  authoritative-choice record (`transitive_impact` vs served
  `reverse_closure`) precedes implementation, as rev2 states.

## Acceptance (unchanged shape)

`execute_attempt` → class-14 discovery from the disclosed accepted-head
binding (actual continuation where needed) → agent mutation → production
admission → real oracle → append → verify; rejection for incomplete
discovery, inconsistent continuation, exceeded budgets, missing evidence; no
private IDs as witness arguments, no hidden manifest, no inventory dump.

- Constraints: read-only review. No file writes, no test execution in the
  reviewer session. Verdict in reply text only, in the REQ verdict format.
