# ADR-0025: Repository exchange composition and clone trust boundary

Status: accepted; the S20-540 contract is frozen at revision 6 (Ariadne and
Vulcan PASS); implementation pending

Date: 2026-09-03

## Context

S20-170 froze a root/object-only pack whose field schema requires empty `refs`
and `transaction_inventory` sets and whose conformance fixture, limits hash,
and decoder are bound by `scripts/check_repository_pack_spec.py`. S20-390 owns
receipts and the single fixed accepted head, and ADR-0021 forbids imported
receipt bytes from becoming commit authority. S20-500 owns immutable
branch-origin records and mutable refs, and ADR-0022 says an origin record
records exact ancestry facts inherited at creation that a replay cannot
reproduce. S20-530 froze the exclusive-recovery matrix over those layers.

S20-540 must reconstruct a clone-equivalent repository in a clean location
from one exchange artifact without weakening any of those boundaries. The
Nabu design consult (session
`forge-nabu-s20-540-design-20260903T013712-0fb98776`) recommended composition
over inline sections, a transaction-owner clone-install API, and byte-exact
branch installation, and raised concerns about nested byte ceilings,
`sley-txn` ownership in the work-package row, interrupted-clone retry, the
definition of an empty target, tombstone and surplus equality, and an
unreachable receipt ceiling.

## Decision

1. **Composition.** Repository Exchange v1 (`docs/spec/REPOSITORY_EXCHANGE_V1.md`,
   contract tag 540, domain `sley2.repository-exchange.v1`,
   `digest_domain_tag` 19) embeds one exact stored S20-170 pack and adds
   receipts, the accepted head, and visible branch pairs as new sections. The
   S20-170 decoder, limits hash, and fixture are untouched. An exchange never
   nests an exchange, and the embedded pack is a single SCB1 `Bytes` field,
   so it is capped at the frozen epoch-1 `Bytes` ceiling of `16,777,216`
   bytes, below the `67,108,864` outer ceiling, with one shared top-down
   allocation budget charged to both decoders.
2. **Trust boundary.** Import targets only a fresh location or an incomplete
   clone of the same `RepositoryExchangeId`, proven by a stage marker that is
   the first regular file written into the target and whose contents are the
   identifier itself; a present head must equal the exchange's head.
   `EXCHANGE_TARGET_NOT_EMPTY` and `EXCHANGE_TARGET_INCOMPLETE_MISMATCH` fire
   before any write. The importer holds exclusive repository maintenance
   ownership for the whole persistence phase under the frozen lock order
   `maintenance -> refs -> accepted`. The fixed accepted head is written last
   and is the sole completion witness; a target without it is not a
   repository to any reader; every persistence step is re-entrant over exact
   bytes. Imported bytes never become authority inside an existing
   repository.
3. **Transaction-owner API.** `sley-txn` gains a two-phase
   `initialize_trusted_clone` boundary (receipts, then head), the explicit
   root-of-trust analogue of `initialize_trusted_genesis`: the caller's
   exclusive maintenance guard plus the exclusive accepted lock, a head that
   is absent or already exactly the exchange's head, parent-before-child
   installation with strict decoding and verification of every receipt
   against the durable store and its durable parent, S20-390 durability
   orders, no candidate or policy authority. The dependency direction stays
   `sley-repo -> sley-txn`. This is transaction-owner work inside the S20-540
   slice, exactly as S20-500 added the verified revision lookup; the
   work-package row names it.
4. **Byte-exact branch install.** `sley-repo` installs exported origin and ref
   records byte-exactly after full verification (fast-forward within the
   exchange's `4,096`-receipt ceiling, facts equal to the named receipts). Replay through
   `create_branch` and `advance_branch` is rejected because it cannot
   reproduce immutable origin records and would multiply invariants.
5. **Equivalence over the exported set.** Clone equivalence is head identity
   and bytes, every verified revision including tombstones, branch list and
   ancestry equality, absence of any surplus receipt, branch, object, marker,
   orphan origin, or temporary, and byte-identical re-export. Orphan origins
   and unreachable receipts are excluded from export, so equivalence is
   defined over the exported set.
6. **Interruption matrix.** Rows X-01 through X-07 freeze the retry behavior
   of an interrupted import; retrying identical bytes converges, retrying
   different bytes fails closed. S20-530 rows are unchanged because an
   incomplete clone is never accepted state.
7. **Limits.** `4,096` receipts and `4,096` branches are clone-profile
   limits consistent with the outer byte ceiling; deeper histories need a
   later streamed or compressed profile.
8. **First contract review.** Ariadne's first review of revision 1
   (session `forge-ariadne-s20-540-contract-20260903T014755-7931f8f9`)
   returned `FAIL_CONTRACT_DRAFT` with four P0 findings (the 32 MiB pack cap
   above the frozen 16 MiB `Bytes` ceiling, raw-name branch order against
   canonical-set order, a marker-after-layout cut that stranded targets, and
   a clone-API precondition that contradicted retry row X-07) plus four P1
   findings (marker contents and foreign heads, an undefined root-closure
   operator and code, untestable multi-genesis, and a double length
   encoding). Revision 2 applies every P0, P1, P2, and P3 item; codes 54020
   and 54021 are added. The second review (session
   `forge-ariadne-s20-540-rereview-20260903T020340-91f5e854`) closed every
   first-review item except the branch order, which it found inexact for
   254- and 255-byte names because of the double varint length prefix, and
   found the marker install non-atomic; revision 3 states the exact length
   order, installs the marker by temp-and-rename, constrains `exchange/v1/`
   contents, and names the two-importer jam consequence. The third pass
   (session `forge-ariadne-s20-540-pass3-20260903T021306-68bdb08f`) returned
   `PASS_CONTRACT_DRAFT`.
9. **Import-surface review and the write guard.** Vulcan's review of
   revision 3 (session
   `forge-vulcan-s20-540-contract-20260903T014755-d837d5c4`) returned
   `FAIL_CONTRACT_DRAFT` with one P1: the claim that a headless incomplete
   clone is not a repository to any reader was unenforced, because
   `initialize_trusted_genesis`, `commit`, `create_branch`, `advance_branch`,
   and GC acquisition operate on a head-absent root, so a third party could
   adopt half-imported receipts and branches under a foreign genesis after a
   crash. Revision 4 makes the stage marker a write guard: those paths fail
   closed with a new S20-390 code `TXN_INCOMPLETE_CLONE` (`39022`), added to
   the frozen transaction error table by the S20-540 slice; read paths stay
   available and establish no acceptance. Revision 4 also re-classifies the
   target after ownership, adds symlink discipline, closed preflight work
   ceilings mirroring the S20-530 recovery limits, one exact code for nested
   exchanges, the receipt ceiling as the fast-forward bound, the inner
   admissibility invariant, and a subset proof for incomplete clones. The
   Vulcan re-review of revision 4 (session
   `forge-vulcan-s20-540-rereview-20260903T022707-896ec8f9`) returned
   `PASS_CONTRACT_DRAFT`; its four text notes (recovery paths must also be
   guarded, `recover_gc_witness` named, object-store puts establish nothing,
   layout creation is not an acceptance write) are applied in revision 5.
   Ariadne's fourth pass (session
   `forge-ariadne-s20-540-pass4-20260903T022707-997681c9`) failed revision 5
   on the guard numeric (already `39022` at HEAD, after the frozen
   `TXN_RESOURCE_LIMIT` `39021`), on an owned re-classification rule that
   would have aborted every fresh import, and on the amendment naming
   `TRANSACTION_MODEL_V1.md` instead of `ERROR_CODES_V1.md` and the enum;
   revision 6 applies those and its P2 and P3 items. The fifth Ariadne pass
   (session `forge-ariadne-s20-540-pass5-20260903T023812-51133a3e`) returned
   `PASS_CONTRACT_DRAFT` on revision 6 with both frozen hashes byte-identical
   to revision 2, and the contract is frozen.
10. **Framing correction (revision 7).** Implementation proved that the
   revision-3 wording for branch order assumed a double length prefix on a
   `Bytes` record field. The frozen SCB1 encoder (`encode_record` over
   `encode_sized`), its decoder, and the independent S20-170 Python oracle
   frame a `Bytes` field once (`uvar(tag) || len(bytes) || bytes`), so the
   canonical-set order of branch entries is plain length-then-bytes for
   every legal name length; the 254 and 255 byte anomaly does not exist.
   Revision 7 restates the paragraph, keeps the 127/128/253/254/255 order
   test, and changes no preimage or hash; it is submitted for a limited
   Ariadne confirmation with the encoder and oracle evidence.

## Consequences

- S20-540 can be frozen, implemented, and reviewed without amending S20-170,
  S20-390, S20-500, or S20-530 contracts.
- `sley-txn` acquires one bounded write path that shares nothing with
  candidate commit and cannot be reached from it.
- A future streamed or compressed exchange profile is a new contract version
  with its own limits and decoder.

## Rejected alternatives

- Inline sections under one new decoder (would re-specify and re-fixture the
  S20-170 sections, duplicating a frozen contract).
- Replaying branch creation and advancement (cannot reproduce origin records;
  more invariants than byte-exact installation).
- Importing into an existing repository or merging exchanged history (an
  authority claim over accepted state; belongs to later merge and fetch
  packages).
- A single outer byte ceiling equal to the embedded pack's ceiling (a full
  S20-170 pack could never be exchanged with any receipt).
