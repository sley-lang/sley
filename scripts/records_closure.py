#!/usr/bin/env python3
"""S20-710 full records-closure model (contract revision 5, section 5).

Distinguishes the attested source candidate commit from a later
records-closure HEAD: a HEAD that advances beyond the candidate commit is
admissible for derivation **only** when the source-to-HEAD diff is provably
records-only:

- every changed tracked path is under ``evidence/`` or ``machineresearch/``
  (anything else -- crates/, scripts/, specs/contracts, lockfiles, build
  inputs -- is attestation-bound and makes the HEAD ineligible), and
- none of the bound inputs changed (the T52 inventory the SPDX namespace
  binds; emitted documents are validated instead by byte-identical
  re-derivation plus the attestation 4-tuple gate).

The SBOM and provenance documents stay bound to the original attested
source candidate: no artifact is rebuilt or re-minted for a permitted
records-only advancement, and the closure HEAD is recorded separately by
the caller (checker output / machine summary), never inside the documents.
This is deliberately not a general commit-skew tolerance: any
attestation-bound change, any bound-artifact change, or any git failure
refuses closed with a records-closure reason.
"""

from __future__ import annotations

import subprocess
from dataclasses import dataclass, field
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]

# Records-eligible prefixes: review/evidence records only.
ELIGIBLE_PREFIXES = ("evidence/", "machineresearch/")

# Bound inputs: derivation inputs under the records prefixes whose change
# re-binds the attestation and therefore ends the closure. (Emitted
# documents -- the SBOM pair, the provenance statement, the
# reproducibility report -- need no byte-invariance: the builders
# re-derive them from the unchanged attestation-bound inputs and refuse
# on any byte difference, and the attestation 4-tuple gate still names
# the candidate. A change to crates/, scripts/, specs/contracts,
# lockfiles, or any other path outside the eligible prefixes is an
# attestation-bound change and is always ineligible.)
BOUND_PATHS = (
    "evidence/security/T52/pre-release-inventory.json",
)


@dataclass
class ClosureStatus:
    """The verdict on one attested-commit-to-HEAD advancement."""

    attested_commit: str
    head: str
    changed: list[str] = field(default_factory=list)
    ineligible: list[str] = field(default_factory=list)
    bound_changed: list[str] = field(default_factory=list)
    git_error: str = ""

    @property
    def is_closure(self) -> bool:
        """A genuine records-closure: advanced, eligible, bound-invariant."""
        return (
            not self.git_error
            and self.head != self.attested_commit
            and not self.ineligible
            and not self.bound_changed
        )

    @property
    def reason(self) -> str:
        """One stable machine-readable refusal/admission marker."""
        if self.git_error:
            return f"records-closure-unverifiable: {self.git_error}"
        if self.head == self.attested_commit:
            return "records-closure-not-advanced"
        if self.bound_changed:
            return (
                "records-closure-bound-changed: "
                + ", ".join(sorted(self.bound_changed))
            )
        if self.ineligible:
            return (
                "records-closure-ineligible: "
                + ", ".join(sorted(self.ineligible))
            )
        return f"records-closure-eligible: {len(self.changed)} records path(s)"


def _git(*args: str) -> str:
    completed = subprocess.run(
        ["git", *args],
        cwd=ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if completed.returncode != 0:
        raise RuntimeError(completed.stderr.strip() or f"git {' '.join(args)} failed")
    return completed.stdout.strip()


def closure_status(attested_commit: str) -> ClosureStatus:
    """Evaluate HEAD against one attested source candidate commit.

    Fail-closed: any git failure yields a non-closure status whose reason
    names the unverifiability, never an admission.
    """
    try:
        head = _git("rev-parse", "HEAD")
    except RuntimeError as error:
        return ClosureStatus(
            attested_commit=attested_commit, head="", git_error=str(error)
        )
    if head == attested_commit:
        return ClosureStatus(attested_commit=attested_commit, head=head)
    try:
        raw = _git("diff", "--name-only", attested_commit, head)
    except RuntimeError as error:
        return ClosureStatus(
            attested_commit=attested_commit, head=head, git_error=str(error)
        )
    changed = [line for line in raw.splitlines() if line]
    ineligible = [
        path
        for path in changed
        if not path.startswith(ELIGIBLE_PREFIXES)
    ]
    bound_changed = [path for path in changed if path in BOUND_PATHS]
    return ClosureStatus(
        attested_commit=attested_commit,
        head=head,
        changed=changed,
        ineligible=ineligible,
        bound_changed=bound_changed,
    )
