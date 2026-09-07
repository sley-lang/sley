# AR-02 measurement probe (RW-075 round-12 input)

Premium delta `reviews/reweave-rw075-premium-r1-2026-09-06.log` AR-02:
canonical identity is single-shot `BLAKE3(domain || preimage)`, so only
the full preimage carries identity; `SLEYCHNK1` hash-of-chunks cannot
carry it; `RAW_HASH_MAX_BYTES` (1 MiB) refuses legally large canonical
preimages. Repair options: cap the bootstrap profile bound, or a
streaming primitive with a domain constant and S20-610-gated
implementation. This probe measures which repair is viable. All facts
verified in-tree at `18ca138`; no code changed.

## 1. Canonical rule (single-shot everywhere native)

`sley-id/src/lib.rs:128-133`: `digest(domain, preimage)` =
single-shot BLAKE3 over `domain.bytes() || preimage`. Every digest type
(`digest_type!` macro, `lib.rs:362-400`: `ObjectId`, `StateRoot`,
`SemanticFingerprint`, `ValueHash`, `ObservationId`, `BytecodeCacheKey`,
all 35 domains in `Domain::ALL`) derives through it. No native path
chunks, streams, or re-frames a canonical preimage.

## 2. Native large-digest paths (canonical, never through RHW1)

| path | ceiling | site |
|---|---|---|
| fingerprint preimage | 67_108_864 B + work 100M | `sley-ssmc/src/fingerprint.rs:21,27` |
| observation preimage | 67_108_864 B | `sley-vm/src/execute.rs:23` |
| image bytes | 67_108_864 B | `sley-vm/src/host_abi.rs:82` |
| bytecode | 67_108_864 B | `sley-vm/src/lower.rs:34` |
| `ValueHash` (`SLEYVHS1` frame, single-shot) | Encoder 64 MiB + work limit | `sley-ssmc/src/fingerprint.rs:282-309` |
| constant payload (SSMC) | 16_777_216 B | `sley-ssmc/src/lib.rs:19` |
| package sections | constants/layouts/dependency 8 MiB, imports 1 MiB, package 64 MiB | `sley-vm/src/exec_package.rs:86-96` |

The only non-test `src` caller of `raw_blake3_256` is the RHW1
execution path (`sley-vm/src/extended.rs:612`, bound enforced at
`:609`). No fingerprint, observation, image, bytecode, or value-hash
preimage is ever routed through RHW1. `image_digest`
(`host_abi.rs:210-214`) is SHA-256 structural binding with no semantic
judgment — out of identity scope by its own contract.

## 3. Sley-side RHW1 preimage inventory (everything executed in-tree)

| workload | preimage | size |
|---|---|---|
| SFP assembly honest + 2 altered (`rw075_raw_callable.rs:1203-1280`) | domain (8 B) + suffix (1 B) | 9 B |
| domain-ownership checks (`:815-880`) | domain prefix + short fields | <= ~60 B |
| raw-hash conformance v1 vectors | whole file 2.5 KB | < 1 KiB each |
| bridge adversarial lanes | tiny lanes + framing | <= ~8 B + framing |
| boundary probes (synthetic fills) | `vec![0x..; N]` | 0 B, 1 B, 1 KiB admit; 1 MiB admits; 1 MiB+1 and 2 MiB refuse code 2 |
| over-bound refusal + framing inertia (`raw_over_bound_refuses_without_composition`) | 1 MiB+1, 2 MiB refuse; `SLEYCHNK1`-framed bytes hash single-shot | retired rule carries no identity (see §5) |

`conformance/bootstrap-profile/v2/accepted.json` (20 vectors) carries
acceptance records, not RHW1 preimages: max hex field is
`bytecode_hex` at 2204 B, hashed natively via the bytecode cache key,
never through RHW1. No real (non-synthetic) workload feeds more than
~1 KiB through RHW1; every over-1 MiB in-Sley case is a synthetic
boundary probe.

## 4. Admission bound (measured, then enforced — round-12 repair)

The gate checked constant value *types* only; `TypeExpr::Bytes`
admitted unconditionally. Round-12 closes the gap: under
`BootstrapProfileVersion::V2` a carried `Bytes` constant over
`RAW_HASH_MAX_BYTES` refuses at admission with `ResourceLimit`
(`bootstrap.rs` constant-inventory loop); V1 needs no bound (no
`RHW1`; `ValueHash` stays canonical under its own encoder/work
limits). Computed over-bound values keep the typed mid-execution
refusal (`Err(Index, 2)`) as backstop. Negative:
`raw_v2_gate_bounds_carried_preimages` (over-bound refuses,
exactly-1 MiB admits, V1 admits the same constant).

## 5. Ungrounded claims found (AR-04 input)

- `docs/spec/BOOTSTRAP_PROFILE_2.md:99`: "All R2-required preimages
  in the adopted closure fixtures measure <1 KiB" — no adopted-closure
  fixture files exist in-tree; the domain list ("hash inventory §1-8")
  lives only in test comments (`rw075_raw_callable.rs:850-880`).
  The <1 KiB property holds for everything actually executed (§3),
  but as stated it cites fixtures that are not present.
- `SLEYCHNK1` is committed as spec law (`BOOTSTRAP_PROFILE_2.md:84-100`)
  plus exactness tests (`:271-355`). The spec itself concedes
  chunk-independent hashing "cannot reproduce BLAKE3 over the
  concatenation". It defines a second, non-canonical digest function
  sharing the digest-carrying channels — the equivocation hazard the
  premium names.

## 6. Decision: cap-the-bound is viable; retire SLEYCHNK1

- (i) Profile law (rides the AR-08 version selector): v2 admission
  refuses carried `Bytes` payloads over 1 MiB; computed over-bound
  values keep the existing typed execution refusal as backstop.
  v1 needs no such bound (no RHW1 after AR-08; `ValueHash` stays
  canonical at any size under its Encoder/work limits).
- (ii) Retire the `SLEYCHNK1` composition rule and its tests
  (keep the refusal assertions: 1 MiB+1 refuses code 2). No
  non-canonical digest function shares the identity channels.
- (iii) Streaming stays an explicit future gap (premium terms:
  its own domain constant, S20-610-gated implementation), never a
  silent redefinition. Nothing R2-required needs it (§3).
- (iv) Restate the <1 KiB claim as a measured suite property with
  this file as evidence, or drop it (AR-04 contract unification).
