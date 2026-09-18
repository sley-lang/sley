# Preserved C0 and C1 candidate

Status: PRESERVED SEED ARTIFACTS (re-minted 2026-09-18 with the arbitrary
RW-090 codec). The exact release-built native seed executable from commit
`e3a50ca7` is preserved read-only outside the repository at the machine
artifact store recorded in `c0-c1-seed-artifacts.json`. Its SHA-256 is
`9c6c5b853e66672c41291d72b1407fd7f9affd510d596390088b4515c4b52389`.

That binary executed its create-once C1 qualification against canonical `S`
(`b1992814cfc2332215d65e1ede6a619fc2ebb8122b8105e5fc527ca8b10548c3`) and
`BOOTSTRAP_PROFILE_2`. In 180.85 seconds it emitted the exact 809,732-byte
`EXEC_PACKAGE_V2` envelope with SHA-256
`87db2921370298422d0c3b4dbd5b2fe28d9f3b93b07329ac4da7a1a2b0eb2f79`.
The contained 766,958-byte image has SHA-256
`fb19ba9123ec0d862fde323034c3f98cdbac2927c5f010544030e40a8d8e6ae7`
and the decoded package digest is
`4d3800996ca8eba63bda1c2a1e5fc10f3c8edad45ed12abc02148753e5637314`.
The reconstruction reached 188 transitive callees in 24,276,433
instructions, 115,671,319 fuel, and 13,958,049,818,064 peak value units. The
envelope the preserved C0 emitted is byte-identical to the one the in-tree
`cargo test --release` run of the same qualification emits.

This is an exact C0-produced C1 candidate, not official C1. C0 legitimately
uses the native seed compiler, and the retained Sley image reconstructs the
whole 189-function closure. The codec member of `S` is now the arbitrary
four-leg codec (both program legs judge arbitrary canonical bodies,
`rw-090-arbitrary-codec-main.md`, `rw-090-arbitrary-canonical-codec-objects.md`).
The candidate still consumes native-prepared lowering facts, and the
integrated checker and lowerer remain bounded; RW-090 independent review is
open. Those gaps prevent the candidate from satisfying the rule that C1
itself owns every required decode, checking, lowering, and orchestration
decision. `bootstrap-manifest.json` therefore records C0 but keeps official
C1 null.

The previous generation — commit `cf210232`, C0
`6bedd5148fa2a7dc10b03255ba7710816d593a26f6e593b376d73dc541176b6f`, C1
candidate `a73fe4cf621826f490adbb50862d3104a99fba56ba06e1613fd52e1672ec865c`,
`S` `4cbcd1eeea202d482895e70ec91f1e0d154c1b7599cc19d60cc6751e61ecfe47`,
101 functions, 179.05 seconds — stays preserved read-only in the same
artifact store under its own commit directories and is recorded as
`superseded_candidate` in `c0-c1-seed-artifacts.json`. Its construction and
qualification tests are retained under the `bounded_*` names.

Validation:

- `python scripts/check_reweave_seed_artifacts.py`
- `python scripts/check_reweave_seed_artifacts.py --require-local`
