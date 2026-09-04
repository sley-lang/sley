# ADR-0048: the hash domain registry covers every crate that hashes

Status: accepted 2026-09-03.

## Context

`IDENTIFIERS_V1.md` freezes the hash domains and states that "adding a domain
requires an ADR, fixtures, and registry drift validation". S20-770 added that
validation, and on 2026-09-03 the registry declared it "carries all
thirty-five domains the implementation derives".

Both halves were wrong in the same way. The check read
`crates/sley-id/src/lib.rs` only, while a domain may be defined by any crate
that hashes, so fifteen domains derived by `sley-repo`, `sley-policy`,
`sley-txn`, and `sley-adapter` were live, specified by their own contracts,
fixtured, and absent from the registry that claimed to be complete.

Domain separation is a security property. The registry is where a reviewer
checks that no two preimages share a domain, and a registry that silently
omits thirty percent of the domains cannot support that review.

## Decision

1. **Every derived domain is registered.** All fifty domains the crates derive
   appear in `IDENTIFIERS_V1.md` with the purpose and owning package.
2. **The drift check reads every crate.** `check_required_contract_index.py`
   compares the registry with the domains derived anywhere under `crates/`, so
   a domain added in any crate fails `make quick` until it is registered.
3. **A test-hook domain is registered and labelled as one.** The S20-530
   recovery ancestry test plan derives a domain that a release build compiles
   out. Hiding it would leave a live domain unreviewed; labelling it states
   what it is.
4. **A randomness domain is registered and labelled as one.** The reference
   adapter's deterministic randomness domain grants no authority and derives
   no identifier, but it is a hash domain and shares the namespace.
5. **The registry states what it covers.** The completeness sentence names the
   number of domains and the scope of the check that keeps it true, so the
   claim and the check cannot drift apart again.

## Consequences

- An independent reviewer sees every domain in one table and can check
  separation without reading five crates.
- Adding a domain anywhere now fails a gate until the registry names it.
- The registry's completeness claim is now a checked property rather than an
  assertion.
