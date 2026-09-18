# RW-120 arbitrary-codec canonical `S` dry run

Date: 2026-09-18

Status: SUPERSEDED BY THE RE-MINTS (same day). The dry run below preceded the codec re-mint recorded in `rw-120-c0-c1-candidate.md`; the canonical `S` was first re-minted as `b1992814…` (pre-review arbitrary generation, e3a50ca7) after the 178873d7 Council repairs as `1d64fcd1…` (7426bc0b), after the 92fa6646 re-review round as `48f8af24…` (0b7a1482), and after the c04539b9 round as `5332d4d7…` (source commit in `canonical-s-manifest.json`), which is the installed canonical `S`. The table below is retained as the dry-run evidence of the first re-mint.

## What was computed

The exact canonical union a codec re-mint would install: the arbitrary
four-leg codec component (`rw-090-arbitrary-canonical-codec-objects.md`,
retained nonce and ordinals) merged with the retained checker, lowerer, and
package-builder components by the same `merged_program` construction, with
the same metadata spine, witnesses, and driver as the retained RW-120
evidence.

| Evidence set | Functions | Parameters | Blocks | Operations | Constants | Objects | Object bytes | Object SHA-256 | Root | Root bytes | Root SHA-256 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- | --- | ---: | --- |
| Retained `S` (bounded codec) | 100 | 5,072 | 1,846 | 4,049 | 699 | 11,786 | 2,949,384 | `a9e6d19a…` | `ca4287c7…` | 778,273 | `b28ab1c1…` |
| Arbitrary-codec `S` | 188 | 8,672 | 3,074 | 6,137 | 676 | 18,767 | 4,725,396 | `20f4a5248a8aa378df951cd24b0e58faa828dd51eaa943ae0a5e3b9e76a9b41b` | `5e4e793b550ea527c5f375222c218c969c8c608beb648941efe9330688a3ca7d` | 1,239,020 | `953420d1039b8ed0730ab78d9b3fe12513405041e2bf16f50b0beccb9c07f5a0` |
| Retained driver `S` | 101 | 5,147 | 1,847 | 4,054 | — | 11,869 | 2,969,080 | `a9f4aaa7…` | `65b567be…` | 783,784 | `396cb4bf…` |
| Arbitrary-codec driver `S` | 189 | 8,747 | 3,075 | 6,142 | — | 18,850 | 4,745,092 | `4a185a532ecfaad23700e558eea9bf1a8bf873b0eb0637b29ba6d24d5f1b985f` | `14595864bd8f1821027848f69c8c865716a5857cbedbd1b1113079db969a93ef` | 1,244,531 | `81bfd9dcfd43737f34979667048a021af68eba67d1d86d003fdfc898c20c6dc5` |

From the arbitrary-codec `S` root, all four real programs execute to their
retained expectations (the integrated checker test, the codec schema-decode
test, the lowerer, and the package builder), every object reimports through
`sley_mutate::import_entity_object`, and the root reimports through the
frozen state-root registry.

The integrated driver over that `S` calls all four programs in one
execution and returns the retained expected value: image 766,500 bytes,
package digest
`7d07a16a564b891310961eaccc1970a21af3366b9e0815d26a17fca28828744a`,
6,142 gate operations, 211 gate bridge uses, 31,223 instructions, 164,581
fuel, 44,187,650 peak value units (retained driver: 490,920-byte image,
4,054 gate operations, 147 bridge uses).

```text
cargo test -p sley-vm --test rw120_toolchain_integration arbitrary_ -- --nocapture
cargo test -p sley-vm --test rw120_toolchain_integration
cargo test -p sley-vm --test rw080_codec_program_outer
```

## Outcome

The re-mint was executed the same day (source commit `e3a50ca7`): the release
seed executable was built and preserved, its create-once qualification over
the handoff `S` (`b1992814…`) emitted the preserved 809,732-byte C1 candidate,
and the manifests and verifier pinned that generation.

Later the same day the 178873d7 Council round (three REVISE lanes) led to
codec repairs — the strict kind-18 route on the canonical decode leg, native
nesting-depth charging at every `TypeExpr`/`ConstValue` site, native-parity
refusal tests for every kind, the fixed-32 trailing-byte parity — and a
second re-mint: handoff `S` `1d64fcd1…` (203 functions, 9,761 parameters,
3,236 blocks, 6,492 operations, 669 constants, 20,382 objects, 5,131,018
bytes, bundle `56b600b1…`, stored root 1,345,643 bytes `8191381e…`), driver
`S` `9533add0…` (203/9,760/3,234/6,488, 20,375 objects), merged component
root `1751abdb…` (202/9,685/3,233/6,483, 20,292 objects). The 92fa6646
re-review round (one shared P2: the constant projector charged `value_type`
from a fixed depth instead of the node depth) led to the third re-mint:
handoff `S` `48f8af24…` (202 functions, 9,763 parameters, 3,236 blocks,
6,492 operations, 669 constants, 20,383 objects, 5,131,404 bytes, bundle
`d7d2fcb5…`, stored root 1,345,709 bytes `76235656…`), driver `S`
`74831205…` (202/9,762/3,234/6,488, 20,376 objects), merged component root
`95f3ca02…` (201/9,687/3,233/6,483, 20,293 objects). The c04539b9 round
(per-arm ConstData boundary sites) found the Map constant arm charging one
nesting level too many and led to the fourth re-mint: the same counts with
handoff `S` `5332d4d7…` (bundle `bee7da94…`, stored root `35256f88…`),
driver `S` `2d8448d8…`, merged component root `f05f2c23…`. The figures in
the table above are the pre-review arbitrary generation, superseded four
times over (bounded → arbitrary → repaired arbitrary → re-review repaired →
arm-parity repaired) under RW080-ID-02 replacement semantics;
`rw-120-c0-c1-candidate.md` and the generation manifests carry the
current values.

The checker and lowerer remain bounded; the codec is arbitrary on both
program legs. Independent re-review of the repairs is pending.
