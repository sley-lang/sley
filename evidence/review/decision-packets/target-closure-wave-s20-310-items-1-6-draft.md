# Owner draft (NOT normative) — S20-310 items 1 and 6 descriptive wording

Status: DRAFT for council review. Not merged into
`docs/spec/ROOT_BACKED_QUERY_PROFILE_V1.md`; no normative effect until
owner + council + operator-text approval. Baseline `4c30d1b`.
S20-310 items 2, 3, 4, 5, 7, 8 remain separately gated (untouched here);
item 9 stays in checker/governance.

## Item 1 — §3 page-set completeness predicate (descriptive)

Proposed §3 append (describes implemented behavior at
`crates/sley-query/src/root_query.rs:582-583,633-638,1322-1391`,
agreed correct per `round-7-root-query-contract.md:20-24`):

> A response is complete standalone iff it was requested with
> `after = None` and reports `truncated = false`. A page set is complete
> iff every page shares one snapshot, root, epoch, workspace, and applied
> limits; page cursors chain (`after` equals the previous page's
> `next_after`); the last page reports `truncated = false`; and the
> `returned` counts sum to `total_count`. Because `total_count` is exact
> on every page and the key order is canonical, a complete page set is
> exactly the complete result: no page can hide a fact.

Mechanical basis: `page_items` (`:1322-1391`) computes exact totals and
canonical key order; truncated pages carry `next_after`; the restricted
`allow_continuation = false` path refuses truncation as
`QUERY_REQUIRED_FACT_OMITTED` (existing §3 text, unchanged).

## Item 6 — §2 paging keys and the strict-increase invariant (descriptive)

Proposed §2 append (describes implemented behavior at
`root_query.rs:261-271` `verify()`, tested by
`unsorted_root_committed_fact_sets_are_root_mismatch` at `:2479-2511`):

> The Key column names each class's paging key: `single` classes never
> page (their whole result must fit the applied limits);
> `entity`-keyed lists page by raw `EntityId`; `root`-keyed lists page by
> raw `StateRoot`; `edge`-keyed lists page by canonical
> `(dependent, dependency, kind)` order. Committed fact sets must present
> strictly increasing, unique keys; an unsorted set is
> `QUERY_ROOT_MISMATCH`, never silently reordered.

Mechanical basis: `verify()` `:267-271` enforces the strict-increase
rule; the named test pins it; §2 Key column (`:103-123`) and §3 rules
(`:207-220`) are the existing normative hooks (unchanged by this draft).

## What council review must confirm

1. Each sentence above is entailed by the cited implementation + tests
   with no additional normative content (descriptive-only).
2. Nothing here touches ownership, composition, dependency direction,
   allocation boundary, failure precedence, or object resolution
   semantics (those are items 2, 3, 4, 5, 7, 8 — excluded).
3. On PASS, the operator approves the exact text; only then does it
   merge into the spec as an editorial clarification.
