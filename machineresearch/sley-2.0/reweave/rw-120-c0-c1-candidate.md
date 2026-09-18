# Preserved C0 and C1 candidate

Status: PRESERVED SEED ARTIFACTS (2026-09-18). The exact release-built native
seed executable from commit `cf210232` is preserved read-only outside the
repository at the machine artifact store recorded in
`c0-c1-seed-artifacts.json`. Its SHA-256 is
`6bedd5148fa2a7dc10b03255ba7710816d593a26f6e593b376d73dc541176b6f`.

That binary executed its create-once C1 qualification against canonical `S`
and `BOOTSTRAP_PROFILE_2`. In 179.05 seconds it emitted the exact 533,671-byte
`EXEC_PACKAGE_V2` envelope with SHA-256
`a73fe4cf621826f490adbb50862d3104a99fba56ba06e1613fd52e1672ec865c`.
The contained 491,378-byte image has SHA-256
`dffdfbbd96585d92a1088f46597ec34bf2271248062a3a84437ba844302552c5`
and the decoded package digest is
`d53bde8d226da2aee57bddeaf093464ef7cbd5f857916dee71aa085c5dcd60e1`.

This is an exact C0-produced C1 candidate, not official C1. C0 legitimately
uses the native seed compiler, and the retained Sley image reconstructs the
whole 101-function closure. The candidate still consumes native-prepared
lowering facts, while the integrated codec and checker remain bounded. Those
RW-090 through RW-120 gaps prevent the candidate from satisfying the rule that
C1 itself owns every required decode, checking, lowering, and orchestration
decision. `bootstrap-manifest.json` therefore records C0 but keeps official C1
null.

Validation:

- `python scripts/check_reweave_seed_artifacts.py`
- `python scripts/check_reweave_seed_artifacts.py --require-local`
