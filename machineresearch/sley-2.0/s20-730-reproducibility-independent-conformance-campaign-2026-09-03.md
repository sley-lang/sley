# S20-730 reproducibility and independent conformance campaign (2026-09-03)

Package: S20-730 (Reproducibility and independent conformance), dependency
S20-720, phase M6. Owner of record Codex; executed by the integrator because
every Council lane was unavailable (ADR-0026). Nothing here releases,
publishes, or claims GA: `make release-check` and `make v2` stay fail-closed.

## Contract

- `docs/spec/REPRODUCIBILITY_AND_INDEPENDENT_CONFORMANCE_V1.md`, draft
  revision 1, with `docs/adr/ADR-0040-reproducibility-and-independent-conformance-boundary.md`.
- Staged checker `scripts/check_reproducibility_and_independent_conformance.py`
  in `make quick`; summary section
  `reproducibility_and_independent_conformance`; work package row updated;
  error codes 73000 through 73007 reserved in `docs/spec/ERROR_CODES_V1.md`.

## Mechanics

| Surface | File | What it does |
|---|---|---|
| Attestation and report | `scripts/build_reproducibility_report.py` | derives this host's attestation from the S20-720 evidence record, merges `--attest` attestations from other hosts, writes `evidence/release/reproducibility-report.json` |
| Coverage report | `scripts/build_independent_conformance_report.py` | derives `evidence/conformance/independent-conformance-report.json` from tracked files: fixture digests, declared oracle command per family, S20-130 oracle independence scan; `--check` detects drift |
| Tests | `bench/release/tests/test_reproducibility.py` | 16 offline tests: attestation derivation, evidence refusals, single- and multi-host results, digest conflicts, duplicate labels, shape refusals, family coverage, recipe agreement, purity, and the three fixture failures |

`make release-candidate-smoke` now rebuilds the reproducibility report after
the candidate build and reruns the staged checker.

## Result at this commit

- Reproducibility: `SINGLE_HOST_REPRODUCIBLE`, one attestation
  (`host_label` "primary") for commit `bdd73f4` with artifact digest
  `6ed337b0c6f77ca607e1ef1574dd9f4e98c159f5ecd5cfe30d771577610b76a4`,
  2,050,866 bytes, 14 members, cargo/rustc 1.93.0. `second_host` is
  `GATED_OPERATOR_LANE`: the laptop is authorized for ZJX performance
  qualification only, so a Sley build there is not authority-safe and is
  recorded as a blocker rather than performed.
- Independent conformance: `INDEPENDENT_CONFORMANCE_PARTIAL`. Nineteen
  fixture families; seventeen are checked by the independent Python oracle
  through named `make conformance` commands; two are native-only, the
  release demo (exercised through the packaged binary) and the extended VM
  vectors (emitted and checked by the Rust VM). The oracle scan finds no
  forbidden marker in its Python sources.

## Open questions for the Council

1. Ariadne: is one attestation per host label the right granularity, or
   should an attestation also bind the toolchain identity into the
   multi-host agreement rule (today only commit and artifact digest must
   agree)?
2. Nabu: should the independent conformance report also record which fuzz
   slices attach to each family, or does that belong to the S20-700 audit?
3. Vulcan: are the two native-only families acceptable for a GA independent
   PASS, or must an independent VM lowering/execution oracle be commissioned
   before S20-740?

## Validation

Landed at `7579d34`. Tier 1 `make quick` passed at the commit, including the
new staged checker, the conformance report drift check, and the sixteen
release tests. Tier 2 ran on 2026-09-03 at that commit:

| Gate | Result | Wall time | Evidence |
|---|---|---:|---|
| `make core` | exit 0 | 13 s | 995 tests passed, 0 failed |
| `make conformance` | exit 0 | 11 s | 19 oracle results PASS |
| `make adversarial` | exit 0 | 9 s | 597 tests passed, 0 failed |
| `make fuzz-smoke` | exit 0 | under 1 s | 5 bounded smoke tests passed |
| `make release-candidate-smoke` | exit 0 | 29 s | two builds REPRODUCIBLE, demo PASS, report rebuilt at `7579d34` (artifact digest `8a76e094...`), staged checker PASS |

The smoke's report rebuild is the intended flow: the tracked reproducibility
report now attests the commit it was built from. `make v1` was not run: this
is a subsystem handoff, not a release boundary.
