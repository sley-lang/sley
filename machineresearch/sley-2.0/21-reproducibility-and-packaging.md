# Reproducibility and Packaging

Status: no released Sley 2 artifact. S20-720 candidate mechanics are
implemented under a draft contract (2026-09-03): `make release-candidate-smoke`
builds `sley-2.0.0-linux-x86_64.tar.gz` twice from the working tree with
remapped paths, records manifest, SHA-256, size, the S20-710 inventory as SBOM,
and declared licenses, unpacks it outside the tree, runs the conformance subset
and the source-independent demo through `bin/sley`, scans for local paths and
secrets, and establishes byte-for-byte reproducibility. The root license,
standards SBOM, provenance, and every publication gate remain closed; the
artifact is a local candidate and the dossier's artifact fields stay null.

S20-720 and S20-730 must build the exact candidate twice, package without
source-tree access or local paths, run the source-independence demonstration,
and record manifest, SHA-256, size, SBOM, licenses, provenance, and any archive-
only nondeterminism. Publication remains separately gated.
