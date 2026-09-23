# Preserved C0 and C1 candidate

Status: PRESERVED SEED ARTIFACTS (re-minted 2026-09-18 with the arbitrary
RW-090 codec, again after the 178873d7 Council repairs, a third time after
the 92fa6646 round, a fourth after the c04539b9 round, and a fifth after the
db53894e round). The exact release-built native seed executable from
commit `8d063f00` (seed sources unchanged since `9d1ba8f1`; the canonical `S` and C1 candidate were first
constructed at `25d6d0ba`; built in a detached clean worktree of that
commit's tree with a fresh target directory and reproduced byte for byte
from a second fresh detached worktree of the same commit) is preserved read-only outside the repository at the machine
artifact store recorded in `c0-c1-seed-artifacts.json`, under
`C0/a94580d7/`. Its SHA-256 is
`a94580d7e96eda2bd0b4c59e72391758e8470ed856f6bde6a1fb4f8521cf1378`
(11,546,744 bytes, build-id `b9d63f63…`; the first build of this generation,
`a51c60ed…` at 25d6d0ba, constructs the same `S` and the same C1 envelope and
is retained — the seed test source changed only in comments after the
76227765 round, which moves the executable's panic line numbers).

That binary executed its create-once C1 qualification against canonical `S`
(`3d2ed8f8bdbc38b4332c11a0cc51d3693e4ba3cdf6f2b15be623ad0a01be6148`) and
`BOOTSTRAP_PROFILE_2`. In 277.52 seconds it emitted the exact 860,220-byte
`EXEC_PACKAGE_V2` envelope with SHA-256
`2e434790836dee0615d68a3cd14ad7390ac49773680a4871dce0e01f2e716417`,
preserved under `C1/2e434790/`.
The contained 817,388-byte image has SHA-256
`f3da10f7427b0eb19851d0f83ef9b75750cd2276437e4507b66ff82ca8af3387`
and the decoded package digest is
`74a56ef552bea91b94da98e73abec32b610c0bc9dbfcd39dce9656df5111a5dc`.
The reconstruction reached 201 transitive callees in 25,885,482
instructions, 123,259,403 fuel, and 15,801,710,950,059 peak value units. The
envelope the preserved C0 emitted is byte-identical to the one the in-tree
`cargo test --release` run of the same qualification emits (both digests
`2e434790…`; the in-tree run's figures are pinned in
`rw120_toolchain_integration.rs`).

This is an exact C0-produced C1 candidate, not official C1. C0 legitimately
uses the native seed compiler, and the retained Sley image reconstructs the
whole 202-function closure. The codec member of `S` is the arbitrary
four-leg codec: both program legs judge arbitrary canonical bodies of all 18
kinds with native-parity refusals and native nesting-depth charging at every
node, every ConstData container arm and every listed sibling after the
178873d7, 92fa6646, c04539b9 and db53894e repairs
(`rw-090-codec-component-manifest.json` `review_repairs`,
`known_deviations` RW090-DEV-01..03, `resource_bound`,
`arm_boundaries_observed`). The candidate still consumes native-prepared
lowering facts, and the integrated checker and lowerer remain bounded
(RW-100, RW-110); the RW-090 independent re-review of this generation is
pending. Those gaps prevent the candidate from satisfying the rule that C1
itself owns every required decode, checking, lowering, and orchestration
decision. `bootstrap-manifest.json` therefore records C0 but keeps official
C1 null.

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

- Fourth-repair arbitrary codec (source commit `867009de`): C0
  `ee7582afe6e1c471d7d7fb8d37cd579bf1dfb2742a9050e46018c1970d0cef25`, C1
  candidate `a329e0bca641e90c6477b06f85204eca17bf5ffb90683a518275dec587559192`,
  `S` `5332d4d758c17c928039cd321ba593aedc5e87f8a57e43ab8e43bc5d877308b3`,
  202 functions, 233.06 seconds — `superseded_candidate` in
  `c0-c1-seed-artifacts.json` (its C0 was reproduced byte for byte from a
  second worktree of 867009de).
- Third-repair arbitrary codec (source commit `0b7a1482`): C0
  `03378aa03709ddb1ab938ec3b21a2e2a9a35e6f061d839cbdf6a6a0989bf1d0e`, C1
  candidate `2b35e6e76fe325b96869c0ca65ac0070a5e98683866042596891ab6a614a0210`,
  `S` `48f8af2422b3e56b2924f56b11ca07f141b87541cdba6584cf1c6e87602577dd`,
  202 functions, 250.63 seconds — `earlier_superseded_candidates[0]` (its C0 was reproduced byte for byte from a
  second worktree of 0b7a1482 and from one of 5bba01b3).
- First-repair arbitrary codec (source commit `7426bc0b`): C0
  `afaa6bef94f4f06b65c5eb3c3dbcb1549178b85641b223d2e443044c87ff06f0`, C1
  candidate `18d1264f8dd352c37305918feefca165fc789f8ec64136211ff92f730998484f`,
  `S` `1d64fcd1298bbab91e8c42a1b673fba683e1a0e2075f560ecfc6aaf849e16c58`,
  203 functions, 242.07 seconds — `earlier_superseded_candidates[1]`. Its C0 was reproduced byte for byte from a
  second fresh detached worktree of 7426bc0b before it was superseded.
- Pre-review arbitrary codec (source commit `e3a50ca7`): C0
  `9c6c5b853e66672c41291d72b1407fd7f9affd510d596390088b4515c4b52389`, C1
  candidate `87db2921370298422d0c3b4dbd5b2fe28d9f3b93b07329ac4da7a1a2b0eb2f79`,
  `S` `b1992814cfc2332215d65e1ede6a619fc2ebb8122b8105e5fc527ca8b10548c3`,
  189 functions, 180.85 seconds — `earlier_superseded_candidates[2]`.
- Bounded codec (source commit `cf210232`): C0
  `6bedd5148fa2a7dc10b03255ba7710816d593a26f6e593b376d73dc541176b6f`, C1
  candidate `a73fe4cf621826f490adbb50862d3104a99fba56ba06e1613fd52e1672ec865c`,
  `S` `4cbcd1eeea202d482895e70ec91f1e0d154c1b7599cc19d60cc6751e61ecfe47`,
  101 functions, 179.05 seconds — `earlier_superseded_candidates[3]`; its
  construction and qualification tests are retained under the `bounded_*`
  names.

All five stay preserved read-only in the same artifact store under their own
directories; `check_reweave_seed_artifacts.py --require-local` verifies every
generation's bytes and modes and the bootstrap manifest's naming of them.

Validation:

- `python scripts/check_reweave_seed_artifacts.py`
- `python scripts/check_reweave_seed_artifacts.py --require-local`
- `python scripts/check_reweave_canonical_s.py`
