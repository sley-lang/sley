# Review request REQ-04 — AT-MW-02 accumulated delta (entity-read surface)

- Baseline tree: `0bcc9c646f110e2e433e8fe3e9b913d49d84c531` (verify: `git rev-parse HEAD` in `/home/greyforge/sley2`). Earlier rounds reviewed `c5973c9`; the repair delta `c5973c9..0bcc9c6` is in scope.
- Delta under review: `e443850..5f5c2dd` (AT-MW-02: entity-read owner, repo adapter, oracle vectors, `conformance/entity-read/v2` fixtures, tests) as modified by the S20-250 repair `cd64f01..7169639` and the two-phase capture follow-up `c5973c9..0bcc9c6`. Review the entity-read surface as it stands at `0bcc9c6`.
- Governing contract: `docs/spec/ENTITY_READ_PROFILE_V2.md` (the surface it defines), composed into the S20-310 record surfaces; `docs/spec/ROOT_BACKED_QUERY_PROFILE_V1.md` carries the prior root-query rounds, which are OUT of scope for entity-read verdicts and must not be superseded by them.
- Register scope: the entity-read surface reads clean apart from the mechanized-comparison evidence gap (now closed by `accepted_corpus_vectors_match_owner_and_encoder`) and record notes below. The standing `root_backed_query_profile` FAIL rounds cover `root_query.rs` (seven P1s, several still open) and stay open regardless of entity-read verdicts.
- Scope:
  - Implementation: `crates/sley-query/src/entity_read.rs`, `crates/sley-repo/src/entity_read.rs`, oracle `entity_read.py`, `conformance/entity-read/v2` (there is no v1 entity-read corpus).
  - Checkers/evidence: entity-read checker + oracle vectors (23 accepted cases reproduced by the Rust owner test; 91 rejections, of which 10 are runtime-sequence obligations without bytes and 36 carry no runtime code; 51 top-level rows are server-path byte vectors), closeout `docs/audits/S20_310_FULL_ROOT_BACKED_QUERY_CLOSEOUT.md`.
- Constraints: read-only review. No file writes. Verdict in reply text only, in the REQ verdict format. Record verdicts against entity-read-scoped fields, never by overwriting the root-query lane dispositions.
