#!/usr/bin/env python3
"""S20-710 full records-closure model (contract revision 5, section 5).

Distinguishes the attested source candidate commit from a later
records-closure HEAD: a HEAD that advances beyond the candidate commit is
admissible for derivation **only** when the source-to-HEAD diff is provably
records-only:

- every changed tracked path is under ``evidence/`` or ``machineresearch/``,
  or is one of the non-normative user guides in ``GUIDE_DOCUMENTATION``
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
import sys
from dataclasses import dataclass, field
from pathlib import Path

try:
    import build_release_candidate
except ImportError:  # loaded by path (unit lane) without scripts/ on sys.path
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import build_release_candidate


ROOT = Path(__file__).resolve().parents[1]

# Records-eligible prefixes: review/evidence records only.
ELIGIBLE_PREFIXES = ("evidence/", "machineresearch/")

# Non-normative user guides. None is an artifact input, a spec, or a
# contract, so editing one cannot change what the candidate built or what
# any checker judges; they may advance past an attested candidate like
# records. Exact paths plus the examples and release-notes directories,
# never all of docs/: docs/spec and docs/adr stay attestation-bound.
GUIDE_DOCUMENTATION = (
    "README.md",
    "docs/README.md",
    "docs/QUICKSTART.md",
    "docs/CONCEPTS.md",
)
# Release notes (docs/release/) are narrative about a build, never an input
# to it: the build's identity lives in evidence/, so a note can be corrected
# after the release commit without a re-mint.
GUIDE_DOCUMENTATION_PREFIXES = ("docs/examples/", "docs/release/")
# GitHub community-health files: funding, issue and pull-request templates,
# conduct, and support. None is read by a build or a checker. Workflows under
# .github/workflows/ stay attestation-bound.
COMMUNITY_FILES = (
    ".github/FUNDING.yml",
    ".github/PULL_REQUEST_TEMPLATE.md",
    "CODE_OF_CONDUCT.md",
    "SUPPORT.md",
)
COMMUNITY_PREFIXES = (".github/ISSUE_TEMPLATE/",)


def is_records_eligible(path: str) -> bool:
    """A path that may change past the attested candidate without a re-mint."""
    return (
        path.startswith(ELIGIBLE_PREFIXES)
        or path in GUIDE_DOCUMENTATION
        or path.startswith(GUIDE_DOCUMENTATION_PREFIXES)
        or path in COMMUNITY_FILES
        or path.startswith(COMMUNITY_PREFIXES)
    )


# Derive bound records from the artifact input surface so both freshness
# checks change together when packaging gains an input. Emitted reports
# are checked by re-derivation rather than treated as artifact inputs.
BOUND_PATHS = tuple(
    path
    for path in build_release_candidate.ARTIFACT_INPUT_PATHS
    if path.startswith(ELIGIBLE_PREFIXES)
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
        if not is_records_eligible(path)
    ]
    bound_changed = [path for path in changed if path in BOUND_PATHS]
    return ClosureStatus(
        attested_commit=attested_commit,
        head=head,
        changed=changed,
        ineligible=ineligible,
        bound_changed=bound_changed,
    )
