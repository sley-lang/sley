# RW-120 arbitrary-codec canonical `S` dry run

Date: 2026-09-18

Status: EVIDENCE ONLY. Nothing here is preserved, no seed executable was built or run, and `bootstrap-manifest.json`, `c0-c1-seed-artifacts.json`, `canonical-s-manifest.json`, and every RW-120 manifest are unchanged. The preserved C0/C1 candidate of `rw-120-c0-c1-candidate.md` remains the retained candidate.

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

## What the re-mint still requires

Installing this `S` as canonical means, in the retained protocol: building
the release native seed executable at the installing commit, running its
create-once C1 qualification against this `S` under `BOOTSTRAP_PROFILE_2`,
preserving the executable and the emitted `EXEC_PACKAGE_V2` envelope
read-only in the machine artifact store, and rewriting
`bootstrap-manifest.json`, `c0-c1-seed-artifacts.json`,
`canonical-s-manifest.json`, and the RW-120 manifests to the new digests
above. That replaces preserved artifacts and is left for the operator to
schedule. The numbers in this record are what that run must reproduce.

The checker and lowerer remain bounded; the codec is now arbitrary on both
program legs. Independent review and acceptance are unchanged.
