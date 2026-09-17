# RW-080 §1.3 package-builder slice 40: composed build entry

Status: PROVISIONAL C0 CONSTRUCTION. This is Sley-owned package construction
under the operator development override. It is not RW-110 completion, C1,
self-hosting evidence, or runtime authority.

## Canonical entry

`package_builder` composes the admitted inventory-section, dependency-section,
and v2-envelope Sley closures behind one function invocation. Its typed inputs
are the lowered `SLEYBC02` image; canonical checked constant, layout, import,
global, and contract row bodies; entry/epoch/root; gate evidence; exact
execution limits; and the five host-mechanic SHA-256 results. It returns
`Result<Bytes, UInt32>` containing the complete `EXEC_PACKAGE_V2` envelope.

The entry calls the generic inventory builder three times with the required
constant versus framed-row mode, calls the dependency builder, extracts its
dependency bytes, and calls the envelope composer. Every child failure is
forwarded unchanged. The independently assembled fixture graphs are rebased
into disjoint artifact and function namespaces before composition; the three
allowlisted byte bridges remain shared dependencies.

## Executable evidence

The one-invocation test uses a real Sley-emitted image, two constants, two
layouts, two imports, one global, one contract, two closure fingerprints, and
a present cancellation fuel. The returned bytes equal the native
`encode_package_envelope_v2` result exactly. Strict hydration reconstructs the
complete input package and all five digests.

The same admitted builder then receives a changed constant value and its new
mechanical digest results. Its envelope changes, again equals the native
reference exactly, and hydrates to the changed package. This is direct
anti-copy evidence for the package stage: the entry does not return a cached
candidate envelope.

## Remaining gate

This closes the bounded §1.3 package-construction remainder recorded by slices
37–39. It does not promote the provisional C0 fixture into C1 or satisfy the
RW-080 contract/surface review. R2 remains NOT_READY under the recorded
independent-review debt; no acceptance is inferred from these tests.
