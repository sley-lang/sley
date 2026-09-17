# Production Fingerprint Profile v1

Status: S1 successor profile implementing the `EPOCH_MIGRATION_POLICY_V1.md`
item 2 determination (`PROFILE`, not an epoch). `VALIDATION_PROFILE_V1.md`
(full-v1) is frozen and unchanged; every epoch-1 artifact still decodes and
still validates under the rule it was accepted with.

## 1. Identity

The production-v1 profile record is byte-identical to the full-v1 record
except `format_version`. The identity derives through the unchanged
construction:

```text
profile_preimage = "SLEYVAP1" || uvar(1) || len(profile_record) || profile_record
ValidationProfileId = BLAKE3-256("sley2.validation-profile.v1" || profile_preimage)
```

| Tag | Field | Exact value |
|---:|---|---|
| 1 | format_version | `2` |
| 2 | phase_tags | ordered list `1..14` |
| 3 | max_operations | `65,535` |
| 4 | max_preconditions | `65,535` |
| 5 | max_candidate_bytes | `67,108,864` |
| 6 | max_decoded_value_bytes | `67,108,864` |
| 7 | max_graph_work | `10,000,000` |
| 8 | max_selected_tests | `65,535` |

The production-v1 identity is pinned:

```text
018eea4824ed41daa782ceaa204a133f15407f994e192d2c3db678fe9998a9f1
```

It is disjoint from the full-v1 identity by construction (the version byte
differs, everything else is equal). No third record is accepted: the
profile-record codec refuses any other byte combination as SCB-invalid, and
candidate build, candidate decode/import, and result import refuse any other
identity with `ValidationProfileInvalid` / `ProfileInvalid`, so the accepted
set is exactly these two identities and never widens silently.

## 2. The requiring rule

The fourteen phases, their order, and every hard ceiling are unchanged from
full-v1. The single additional rule: under the production-v1 identity, every
`TypeDef` and `Function` entity in the proposed closure must carry a present
semantic-fingerprint claim that recomputes exactly.

- Phase 6 recomputes each proposed `TypeDef` claim after its owning S20-210
  checker passes. An absent claim fails the phase with terminal decision
  `TypeError` and source symbol `FINGERPRINT_CLAIM_MISSING` (numeric 25003,
  permanent). A present but inexact claim keeps the existing
  `FINGERPRINT_MISMATCH` path unchanged.
- Phase 8 recomputes each proposed `Function` claim after the S20-230 effect
  report. An absent claim fails the phase with terminal decision
  `EffectError` and source symbol `FINGERPRINT_CLAIM_MISSING` (numeric 25003,
  permanent). A present but inexact claim keeps the existing
  `FINGERPRINT_MISMATCH` path unchanged.
- Present claims on an unsupported entity kind still fail closed, exactly as
  under full-v1.
- Candidates with no `TypeDef` or `Function` entities validate identically
  under both identities; the requirement constrains claims, not entity kinds.

## 3. Records and decoder

- `validation_profile_id` (candidate field 11, result field 4) stays
  `FixedBytes<32>`; the decoder does not change. A result carries the
  identity the candidate requested; result import accepts exactly the two
  pinned identities and refuses any other value with `ProfileInvalid`.
- A profile ID is not evidence that its phases ran; the candidate result
  must carry exact phase evidence under S20-360, exactly as under full-v1.
- Transaction binding checks compare the transaction's profile field against
  the candidate and result records for equality; they are profile-agnostic
  and unchanged.

## 4. Explicit exclusions

- No epoch is created, proposed, or migrated; schema epoch 1 is untouched.
- No commit or merge path constructs production-v1 candidates; assembly
  under this profile and any GA claim are later successor work, still
  blocked on the remaining S-slice evidence.
- No change to full-v1 bytes, identity, phases, ceilings, corpora, oracles,
  or frozen vectors.
