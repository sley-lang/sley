# Review request REQ-10 rev4 — CONTEXT bounded discovery contract delta

- Supersedes rev3 (sha256 `b817d219...798d974`; verdict REVISE narrowed to
  one reachability issue with two prerequisite P2s and one P3 anchor fix;
  transcript `evidence/review/verdicts/context_bounded_discovery/nabu_architecture_review-rev3-b0e52e00.md`).
  Rev3's option-B withdrawal, omission-not-refusal shape, and P2 ownership
  gate stand as reviewed.
- Baseline tree: `b0e52e00a2bacfd7a874f0b4a7973a352508cebe` on
  `work/succession-sley20-arm`. Lane: Nabu (architecture).
  Section `context_bounded_discovery`, field `nabu_architecture_review`.
- Transport note: the rev3 verdict relay truncated two lines at 2000 chars
  (P1 remedy list after preference "(a)" and the SUMMARY tail). Rev4 follows
  remedy (a) as the stated first preference; if a lost option was preferred,
  the reviewer says so and rev4 is re-derived.

## Answers (single issue + prerequisites)

- P1 (reachability): adopt remedy (a) — afford `workspace.open` to the arm
  and carry the accepted-head snapshot identity in its response. Explicit
  allowlist change: move `workspace.open` from `ARM_DENIED_METHODS`
  (`runner.py:143`) to `ARM_AFFORDANCES`; the claim digest moves accordingly
  (auditable run-control change; the allowlist owner's decision, flagged as
  such). `workspace.open` already pins the accepted head (it is the opener
  the live tooling uses, `sley2_tool.py:258-264`) and returns the head
  `revision_summary`; field 9 is populated there for the head it opens.
  Disclosure and discovery therefore pin one revision by construction, and
  the arm obtains the binding through an allowed route — no silent absence,
  no new symbol. The `refs.resolve` branch-head path is untouched.
- P2 (probe): specify the entry point — a new public probe in `sley-repo`
  (`read_record` + `accept_cached`, no `fresh_snapshot` fallthrough), owned
  by the index domain; field 9 is derived from it. Until the probe exists,
  no implementation. With it, the O(1)/`returned_entities = 1` accounting
  holds truthfully and the deferred Vulcan audit sees exactly what runs.
- P2 (placement): resolved in this lane, not deferred — disclosure stays on
  the head-pinned opener route (`workspace.open`), not grafted onto
  arbitrary-revision metadata (`revision.read`). Ariadne receives only the
  resulting profile text in the deferred composition review.
- P3 (anchor): `server.rs:2685` (`let revision = self.head()?`) is the
  `query_root` head pin, not `:2665`; `repository.rs:881-905` and
  `QUERY_SNAPSHOT_MISMATCH` at `server.rs:2697-2699` stand as cited.

## Acceptance (unchanged shape, now reachable)

`workspace.open` (allowed) → accepted-head snapshot binding → class-14
discovery with actual continuation where needed → agent mutation → production
admission → real oracle → append → verify; rejection for incomplete
discovery, inconsistent continuation, exceeded budgets, missing evidence; no
private IDs as witness arguments, no hidden manifest, no inventory dump.

- Constraints: read-only review. No file writes, no test execution in the
  reviewer session. Verdict in reply text only, in the REQ verdict format.
