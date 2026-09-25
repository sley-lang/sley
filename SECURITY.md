# Security and Threat Model

## Reporting a vulnerability

Please report security issues privately. Use GitHub's private vulnerability
reporting: the **Security** tab of
[sley-lang/sley](https://github.com/sley-lang/sley/security), then
**Report a vulnerability**. Don't open a public issue for a suspected
vulnerability. Include the affected version or commit, a minimal reproduction
(SMP1 frames, bytes, or a failing test), and the impact you observed. The
severity scale below is the one we triage with.

Status (2.0.1): the threat register in
[`docs/THREAT_REGISTER.md`](docs/THREAT_REGISTER.md) maps each of the 56
threats to an owner, a failure code, a required test, and an evidence path.
Its implementation evidence is measured by
`scripts/build_threat_coverage_report.py`, which writes
`evidence/security/threat-coverage-report.json`. The independent security
review of that evidence is still pending, so no threat is claimed as
mitigated in the GA sense. See [Threat register](#threat-register) below.

## Security invariants

The kernel fails closed. Unknown, incomparable, ambiguous, missing, stale, or
over-limit facts never imply validity or permission. Opaque binary encoding is
not a security boundary. Prompt text, labels, documentation, adapter text, and
model output cannot grant authority.

Policy, schema epoch, validator, kernel version, and mandatory oracle changes
are isolated from the candidate they judge. There is no ambient filesystem,
network, clock, randomness, environment, process, secret, deployment, or spend
authority. Arbitrary shell execution is outside Sley 2.0 GA.

## Severity and evidence

- P0: integrity or authority failure that can commit invalid state or escape a
  capability boundary.
- P1: exploitable denial, cross-workspace leak, durable corruption, or release
  substitution.
- P2: material hardening or architecture debt; GA blocks while open.
- P3/P4: non-blocking improvement or observation.

Every P0/P1 row must gain a mitigation, stable error code, deterministic,
property, or fuzz test, evidence path, and independent disposition before GA.

## Threat register

The one-to-one owner/code/test/evidence map is `docs/THREAT_REGISTER.md`. The
grouped view below is only a navigation summary.

| IDs | Threat family | Initial severity | Required control and later evidence |
|---|---|---:|---|
| T01-T06 | malformed/noncanonical SCB1, hash substitution, downgrade, epoch confusion | P0 | strict decoder, domain hashes, epoch pinning, Rust/oracle rejection corpus |
| T07-T12 | identity reuse, dangling/cyclic/pathological graph, checker nontermination | P0 | identity ledger, bounded traversal, graph/type/CFG property and fuzz suites |
| T13-T16 | query explosion, hidden truncation, stale handles, mutation flooding | P1 | exact limits, omission markers, root-bound handles, resource tests |
| T17-T21 | stale commit and self-modified policy/epoch/tests | P0 | exact preimages, protected roots, phase-ordered candidate tests |
| T22-T26 | capability forgery/replay/scope confusion and adapter impersonation/injection | P0 | authenticated root-bound tokens, typed adapter identity, adversarial fixtures |
| T27-T32 | path/symlink escape, environment/output/cancellation/fuel bypass | P0 | confined adapters, no ambient state, hard budgets, process isolation tests |
| T33-T36 | VM divergence, floating drift, cache poisoning, derived-as-canonical | P0 | frozen FP profile, exact cache keys, VM conformance and fault seeding |
| T37-T40 | crash boundaries and GC deleting reachable objects | P0 | write ordering, fsync/CAS, crash matrix, reachability property tests |
| T41-T44 | malicious/decompression pack and lossy/silent merge | P0 | bounded pack decode, digest tree, explicit conflict objects, merge properties |
| T45-T47, T56 | downgrade, request confusion, cross-workspace leakage, session name used by a non-opening caller | P0/P1 | explicit negotiation, typed IDs, session/workspace binding tests, per-instance session names |
| T48-T51 | prompt/debug/Git/ZJX facts mistaken for semantics | P0 | semantic authority boundary and negative conformance fixtures |
| T52-T55 | dependency/artifact substitution, secrets, benchmark contamination | P1 | lockfile, SBOM, provenance, secret scan, frozen corpus and failure retention |

The register's main table is the original plan: it names the failure code
each control was expected to produce. Its "Realized codes" addendum records
the codes the controls actually shipped under, and where each is enforced and
exercised. The coverage report classifies every threat by what it can locate
in the tree. At the 2.0.0 release it recorded 44 threats with a located
control and a test that exercises it, 3 structural controls (a control that
is the absence of something, such as ambient environment access), and 9 with
their planned evidence directory present. A located control is traceability,
not a mitigation claim. The judgment stays with the independent security
review.
