# Preserved C0 and C1 candidate

Status: PRESERVED SEED ARTIFACTS (re-minted 2026-09-18 with the arbitrary
RW-090 codec, and again the same day after the 178873d7 Council repairs). The
exact release-built native seed executable from the commit carrying
`c0-c1-seed-artifacts.json` (built in a detached clean worktree of that tree
with a fresh target directory) is preserved read-only outside the repository
at the machine artifact store recorded in `c0-c1-seed-artifacts.json`, under
`C0/afaa6bef/`. Its SHA-256 is
`afaa6bef94f4f06b65c5eb3c3dbcb1549178b85641b223d2e443044c87ff06f0`
(11,494,520 bytes, build-id `8f347e69…`).

That binary executed its create-once C1 qualification against canonical `S`
(`1d64fcd1298bbab91e8c42a1b673fba683e1a0e2075f560ecfc6aaf849e16c58`) and
`BOOTSTRAP_PROFILE_2`. In 242.07 seconds it emitted the exact 860,150-byte
`EXEC_PACKAGE_V2` envelope with SHA-256
`18d1264f8dd352c37305918feefca165fc789f8ec64136211ff92f730998484f`,
preserved under `C1/18d1264f/`.
The contained 817,286-byte image has SHA-256
`3e51ae323d69e7c287962834ebc7fa82dc47558fd81cd5e25487e71c84ee3189`
and the decoded package digest is
`29cd6f2a4a5b3a1498b5958447a217367075fa68c3d7b7d8004980ee5443d3c8`.
The reconstruction reached 202 transitive callees in 25,881,022
instructions, 123,242,045 fuel, and 15,798,780,873,178 peak value units. The
envelope the preserved C0 emitted is byte-identical to the one the in-tree
`cargo test --release` run of the same qualification emits (both digests
`18d1264f…`).

This is an exact C0-produced C1 candidate, not official C1. C0 legitimately
uses the native seed compiler, and the retained Sley image reconstructs the
whole 203-function closure. The codec member of `S` is the arbitrary
four-leg codec: both program legs judge arbitrary canonical bodies of all 18
kinds with native-parity refusals and native nesting-depth charging after
the 178873d7 repairs (`rw-090-codec-component-manifest.json`
`review_repairs`, `known_deviations`, `resource_bound`). The candidate still
consumes native-prepared lowering facts, and the integrated checker and
lowerer remain bounded (RW-100, RW-110); the RW-090 independent re-review of
the repairs is pending. Those gaps prevent the candidate from satisfying the
rule that C1 itself owns every required decode, checking, lowering, and
orchestration decision. `bootstrap-manifest.json` therefore records C0 but
keeps official C1 null.

## Seed lane and the records-only rule

The seed lane's own rule, stated here because the release attestation lane's
records-only prefixes (`records_closure.py`) do not govern it: C0 is built
from the tree of the commit that carries the seed manifest (the tree, not the
commit hash, determines the bytes: no build script embeds git identity); the
seed manifest, the canonical `S` manifest and the bootstrap manifest are
committed together with the seed-constructor sources they bind, and the
verifier `check_reweave_canonical_s.py` binds the complete `#[path]` closure
of the seed binary, so any later source edit is visible. Only the commit
identity itself and the rebuild verification are appended by the records-only
descendant. The 2026-09-18 pre-review re-mint (`e3a50ca7`) had committed the
verifier pin update in the re-mint commit and later source edits (a7852028)
after it; both are recorded in the transcripts of the 178873d7 round and
superseded by this generation.

## Superseded generations

- Pre-review arbitrary codec (source commit `e3a50ca7`): C0
  `9c6c5b853e66672c41291d72b1407fd7f9affd510d596390088b4515c4b52389`, C1
  candidate `87db2921370298422d0c3b4dbd5b2fe28d9f3b93b07329ac4da7a1a2b0eb2f79`,
  `S` `b1992814cfc2332215d65e1ede6a619fc2ebb8122b8105e5fc527ca8b10548c3`,
  189 functions, 180.85 seconds — `superseded_candidate` in
  `c0-c1-seed-artifacts.json`.
- Bounded codec (source commit `cf210232`): C0
  `6bedd5148fa2a7dc10b03255ba7710816d593a26f6e593b376d73dc541176b6f`, C1
  candidate `a73fe4cf621826f490adbb50862d3104a99fba56ba06e1613fd52e1672ec865c`,
  `S` `4cbcd1eeea202d482895e70ec91f1e0d154c1b7599cc19d60cc6751e61ecfe47`,
  101 functions, 179.05 seconds — `earlier_superseded_candidates[0]`; its
  construction and qualification tests are retained under the `bounded_*`
  names.

Both stay preserved read-only in the same artifact store under their own
directories; `check_reweave_seed_artifacts.py --require-local` verifies every
generation's bytes and modes and the bootstrap manifest's naming of them.

Validation:

- `python scripts/check_reweave_seed_artifacts.py`
- `python scripts/check_reweave_seed_artifacts.py --require-local`
- `python scripts/check_reweave_canonical_s.py`
