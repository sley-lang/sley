# Review request REQ-05 — entity-read surface, scoped lanes

- Baseline tree: `9ae09a142830a4857c553bad27433143999e6864` (verify: `git rev-parse HEAD` in `/home/greyforge/sley2`).
- Scope: the S20-310 entity-read surface only — `crates/sley-query/src/entity_read.rs` (query-owned narrow views, borrowed selection, post-reservation capture), `crates/sley-repo/src/entity_read.rs` (adapter projection), `oracle/scb1/src/sley2_scb1_oracle/entity_read.py` (pure-Python HEAD with commondir, promote-safe manifests), `conformance/entity-read/v2` (23 accepted vectors reproduced by the Rust owner test `accepted_corpus_vectors_match_owner_and_encoder`; 91 rejections oracle-checked).
- Governing contract: `docs/spec/ENTITY_READ_PROFILE_V2.md`, composed into the S20-310 record surfaces by `docs/spec/ROOT_BACKED_QUERY_PROFILE_V1.md` revision 4 section 11 (checker `scripts/check_root_backed_query_profile.py` binds the composition).
- Register fields: `root_backed_query_profile.ariadne_entity_read_review`, `.nabu_entity_read_review`, `.vulcan_entity_read_review` (all PENDING — first review).
- Explicitly out of scope: the nineteen-class `root_query.rs` surface and its standing FAIL rounds with open P1s (cursor-before-drift, walk-per-class, arm rule, paging keys, absent vectors), which are unrepaired and stay open on the root-query lane dispositions. A verdict here must not supersede them.
- Prior REQ-04 REVISE rounds (record binding) are closed by the revision-4 composition above; the seq_root_advanced vector now expects SESSION_ROOT_ADVANCED 33002; the Rust-vs-vector gap is closed mechanically.
- Constraints: read-only review. No file writes. Verdict in reply text only, in the REQ verdict format, against the `*_entity_read_review` field named per lane.
