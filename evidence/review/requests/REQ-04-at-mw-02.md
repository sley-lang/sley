# Review request REQ-04 — AT-MW-02 accumulated delta (entity-read surface)

- Baseline tree: `c5973c90180d03402f0d2e5d2d91e941ef5dc58d` (verify: `git rev-parse HEAD` in `/home/greyforge/sley2`).
- Delta under review: `afb5627..cd64f01` (AT-MW-02: entity-read owner, repo adapter, oracle vectors, `conformance/entity-read/v1` fixtures, tests) as modified by the S20-250 repair `cd64f01..7169639` in the same files. Review the entity-read surface as it stands at `c5973c9`.
- Register section: `root_backed_query_profile` (S20-310 full root-backed query).
- Lanes/fields: `ariadne_contract_review`, `nabu_architecture_review`, `vulcan_surface_review` (prior FAIL rounds with P0s; AT-MW-02 implementation + S20-250 repair landed since — re-review for same-lane superseding verdict).
- Scope:
  - Implementation: `crates/sley-query/src/entity_read.rs`, `crates/sley-repo/src/entity_read.rs`, oracle `entity_read.py`, `conformance/entity-read/v1`.
  - Contract: `docs/spec/ROOT_BACKED_QUERY_PROFILE_V1.md`.
  - Checkers/evidence: entity-read checker + oracle vectors (23 cases / 91 rejections), closeout `docs/audits/S20_310_FULL_ROOT_BACKED_QUERY_CLOSEOUT.md`.
- Lane focus: ariadne = S20-310 contract conformance of the read path; nabu = ownership/layering of owner vs adapter vs oracle; vulcan = fixture/vector coverage and failure precedence.
- Constraints: read-only review. No file writes. Verdict in reply text only, in the REQ verdict format.
