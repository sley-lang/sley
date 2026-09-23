#!/usr/bin/env python3
"""Check the S20-330 negotiated session and handle contract and its stage."""

from __future__ import annotations

import json
import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SPEC = ROOT / "docs/spec/SESSION_HANDLE_PROFILE_V1.md"
ADR = ROOT / "docs/adr/ADR-0033-negotiated-session-boundary.md"
WORK_PACKAGES = ROOT / "docs/WORK_PACKAGES.md"
SUMMARY = ROOT / "machineresearch/sley-2.0/machine-summary.json"
ERROR_CODES = ROOT / "docs/spec/ERROR_CODES_V1.md"
SESSION_MODULE = ROOT / "crates/sley-protocol/src/session.rs"
SERVER_MODULE = ROOT / "crates/sley-protocol/src/server.rs"
SERVER_TESTS = ROOT / "crates/sley-protocol/src/server_tests.rs"
REGISTRY_MODULE = ROOT / "crates/sley-protocol/src/lib.rs"
CAPSULE_MODULE = ROOT / "crates/sley-query/src/context_capsule.rs"
ID_CRATE = ROOT / "crates/sley-id/src/lib.rs"
SMP1_SPEC = ROOT / "docs/spec/SMP1.md"
CAPSULE_SPEC = ROOT / "docs/spec/CONTEXT_CAPSULE_PROFILE_V1.md"
EVIDENCE = ROOT / "evidence/security"
THREAT_MATRICES = ("T15", "T47", "T56")

DRAFT_STATUS = "S20_330_CONTRACT_DRAFT_REVIEW_PENDING"
DRAFT_IN_PROGRESS_STATUS = "S20_330_CONTRACT_DRAFT_IMPLEMENTATION_IN_PROGRESS"
FROZEN_STATUS = "S20_330_CONTRACT_FROZEN"
IMPLEMENTED_STATUS = "S20_330_CONTRACT_DRAFT_S20_410_IMPLEMENTED_REVIEW_PENDING"
REVIEW_PENDING_STATUS = "S20_330_IMPLEMENTED_REVIEW_PENDING"
COMPLETE_STATUS = "S20_330_COMPLETE"
IMPLEMENTATION_STATUSES = (
    DRAFT_IN_PROGRESS_STATUS,
    IMPLEMENTED_STATUS,
    REVIEW_PENDING_STATUS,
    COMPLETE_STATUS,
)

CONTRACT_REVISION = 4
# The composed authorities' current revisions. Each is cross-checked
# against that document's own status line, so the pin fails the moment
# the authority moves instead of matching a stale substring elsewhere.
SMP1_REVISION = 13
CAPSULE_REVISION = 4
SMP1_PIN = f"`docs/spec/SMP1.md` at revision {SMP1_REVISION}"
CAPSULE_PIN = (
    f"`docs/spec/CONTEXT_CAPSULE_PROFILE_V1.md` at revision {CAPSULE_REVISION}"
)

CODES = (
    (33000, "SESSION_UNKNOWN"),
    (33001, "SESSION_WORKSPACE_MISMATCH"),
    (33002, "SESSION_ROOT_ADVANCED"),
    (33003, "SESSION_EPOCH_MISMATCH"),
    (33004, "SESSION_STALE_HANDLE"),
    (33005, "SESSION_HANDLE_UNKNOWN"),
    (33006, "SESSION_RENEWAL_LIMIT"),
    (33007, "SESSION_BINDING_INVALID"),
)
# Contract section 3 classifies every frozen method tag into exactly one
# list; the anchors are the list headings.
HEAD_BOUND_ANCHOR = "Head-bound methods (checked for the bound root, item 5):"
CLASS_ANCHORS = (
    HEAD_BOUND_ANCHOR,
    "Handle expansion (checked for the bound root by its own comparison,",
    "Caller-named methods (answer over state the request names, never the",
    "Mutating methods (advance the head instead of answering over it):",
    "Session and transport methods (answer over the session, never the",
)
SESSION_OPEN_TAG = 100
SPEC_MARKERS = (
    "# Negotiated Session and Handle Profile v1",
    "`sley2.session.v1 -> SessionId`",
    "ServerNonce[32] || ProtocolHandshakeId[32]",
    "## 2. Session record and issuance",
    "renewals:      u16",
    "## 3. Request checks",
    "The head-bound set is closed.",
    "## 4. Handles",
    "uvar(handle) || StateRoot[32] expected_root",
    "## 5. Capsule binding",
    "`SessionBinding = Negotiated(2) || SessionId[32]`",
    "## 8. Explicit exclusions",
    "at most `max_sessions` remembered",
    "handles naming query cursors",
    "Protocol version 2 extension",
    "## 9. Revision history",
    "threat T56",
    SMP1_PIN,
    CAPSULE_PIN,
) + CLASS_ANCHORS
ADR_MARKERS = (
    "# ADR-0033: Negotiated session, binding checks, and positional handles",
    "1. **A session binds a handshake to one workspace, root, and epoch,",
    "2. **The identity-domain choice is unpredictability, keeping",
    "3. **Every request is checked against the binding in true precedence.**",
    "4. **Handles are positions under an expected root, not bare numerals.**",
    "5. **Genesis is the only sessionless state.**",
    "6. **Codes.**",
    "7. **Staging.**",
)
WORK_PACKAGE_MARKERS = ("`docs/spec/SESSION_HANDLE_PROFILE_V1.md`", "ADR-0033")
SESSION_MARKERS = (
    "pub struct SessionRecord",
    "pub renewals: u16,",
    "pub const MAX_SESSION_RENEWALS: u16",
    "pub fn open_session",
    "pub fn renew_session",
    "pub fn check_session",
    "pub fn expand_handle",
    "pub fn bind_context_capsule",
    "pub fn fresh_server_nonce",
    "server_nonce",
    "expected_root: StateRoot",
    "max_sessions: u32",
)
SERVER_MARKERS = (
    "head_binding().is_ok()",
    "is_closed(session)",
    "fixed32(body)? != *session.as_bytes()",
    "close(session, self.profile.limits.max_sessions)",
    "expected_root",
    ".bind_context_capsule(session, &outcome.request, &outcome.response)",
)
REGISTRY_MARKERS = (
    "pub fn is_closed",
    "close_order",
    "MAX_LIMIT_SESSIONS",
    "max_sessions",
)
CAPSULE_MARKERS = ("pub fn build_context_capsule_session", "Negotiated")
ID_MARKERS = ('b"sley2.session.v1"', "digest_type!(SessionId, Domain::Session);")
# Implemented under a draft means tracked, never silently pending: a FAIL
# round is itemized in same-lane register-first open lists, or superseded
# by the same lane's re-review PASS obligation (S20-740 register rule).
LANE_PASS_FIELD = {
    "ariadne_contract_review": "ariadne_review",
    "nabu_architecture_review": "nabu_review",
    "vulcan_surface_review": "vulcan_review",
}
LANE_PREFIX = {
    "ariadne_contract_review": "Ariadne ",
    "nabu_architecture_review": "Nabu ",
    "vulcan_surface_review": "Vulcan ",
}
OPEN_LISTS = ("p1_open", "p2_open", "p3_open")


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def rust_block(text: str, opener: str) -> str:
    """The body of the first Rust item starting with `opener`."""
    start = text.find(opener)
    if start < 0:
        return ""
    end = text.find("\n    }", start)
    return text[start:end] if end > start else text[start:]


def spec_paragraph(spec: str, anchor: str) -> str:
    start = spec.find(anchor)
    if start < 0:
        return ""
    end = spec.find("\n\n", start)
    return spec[start:end] if end > start else spec[start:]


def check_method_classification(
    spec: str, server: str, registry: str, smp1: str, problems: list[str]
) -> dict[str, list[int]]:
    """Section 3 lists against the server dispatch table and SMP1 table.

    The version 1 partition is compared exactly; the version 2 partition is
    the version 1 partition with the two entity-read tags added to the
    head-bound list only. Numeric tag constants declared as named constants
    (the entity-read tags) are resolved exactly; an unresolvable mapped
    variant fails instead of being skipped.
    """
    tag_block = rust_block(registry, "pub const fn tag(self) -> u32 {")
    const_values = {
        name: int(value) for name, value in re.findall(r"pub const (\w+): u32 = (\d+);", registry)
    }
    tag_of: dict[str, int] = {}
    for name, value in re.findall(r"Self::(\w+) => ([^,]+),", tag_block):
        value = value.strip()
        if re.fullmatch(r"\d+", value):
            tag_of[name] = int(value)
        elif value in const_values:
            tag_of[name] = const_values[value]
        else:
            problems.append(f"classification:unresolved-tag-const:{name}:{value}")
    # Declared entity-tag constants resolve completely to their frozen tags.
    for const, expected in (("ENTITY_VERSION_TAG", 306), ("ENTITY_SIGNATURE_TAG", 307)):
        if const_values.get(const) != expected:
            problems.append(f"classification:entity-tag-const:{const}")
    reserved = {
        tag_of[name]
        for name in re.findall(
            r"Self::(\w+)",
            rust_block(registry, "pub const fn is_reserved(self) -> bool {"),
        )
        if name in tag_of
    }

    def method_array(source: str, opener: str) -> set[str]:
        start = source.find(opener)
        if start < 0:
            return set()
        end = source.find("];", start)
        return set(re.findall(r"Self::(\w+)", source[start:end] if end > start else source[start:]))

    all_names = method_array(registry, "pub const ALL: [Self; 41] = [")
    v2_names = method_array(registry, "pub const V2_ALL: [Self; 43] = [")
    if len(all_names) != 41:
        problems.append(f"classification:all-count:{len(all_names)}")
    if len(v2_names) != 43:
        problems.append(f"classification:v2-all-count:{len(v2_names)}")
    if all_names and v2_names and v2_names - all_names != {"EntityVersion", "EntitySignature"}:
        problems.append(
            f"classification:v2-additions:{sorted(v2_names - all_names)}"
        )
    server_head_bound = {
        tag_of[name]
        for name in re.findall(
            r"Method::(\w+)",
            rust_block(server, "const fn head_bound(method: Method) -> bool {"),
        )
        if name in tag_of
    }
    # The versioned helper delegates to the legacy helper, so its partition
    # is the legacy set union its explicit additions, never a partial parse.
    # The admitted shape is the known disjunction of the legacy call and a
    # matches! over exactly the two additions; any other Boolean form or
    # altered delegation refuses.
    versioned_block = rust_block(server, "const fn head_bound_versioned(method: Method) -> bool {")
    if "Self::head_bound(method)" not in versioned_block:
        problems.append("classification:versioned-delegation")
    if not re.search(
        r"\{\s*Self::head_bound\(method\)\s*\|\|\s*matches!\s*\(\s*method\s*,"
        r"\s*Method::(?:EntityVersion\s*\|\s*Method::EntitySignature|EntitySignature\s*\|\s*Method::EntityVersion)"
        r"\s*\)\s*,?\s*$",
        versioned_block,
        flags=re.DOTALL,
    ):
        problems.append("classification:versioned-delegation-shape")
    versioned_additions = {
        tag_of[name]
        for name in re.findall(r"Method::(\w+)", versioned_block)
        if name in tag_of
    }
    for name in re.findall(r"Method::(\w+)", versioned_block):
        if name not in tag_of:
            problems.append(f"classification:unresolved-versioned-variant:{name}")
    server_head_bound_versioned = server_head_bound | versioned_additions
    smp1_rows = dict(re.findall(r"^\| (\d{3}) \| `([a-z_.]+)` \|", smp1, flags=re.M))
    lists: dict[str, list[int]] = {}
    for anchor in CLASS_ANCHORS:
        paragraph = spec_paragraph(spec, anchor)
        pairs = re.findall(r"`([a-z_.]+)`\s+\((\d+)", paragraph)
        if not pairs:
            problems.append(f"classification:empty:{anchor[:20]}")
        tags: list[int] = []
        for name, tag in pairs:
            numeric = int(tag)
            tags.append(numeric)
            if smp1_rows.get(tag) != name:
                problems.append(f"classification:smp1-name:{name}:{tag}")
        lists[anchor.split(" ", 1)[0].lower()] = tags
    head_bound = set(lists.get("head-bound", []))
    if not tag_of or not server_head_bound:
        problems.append("classification:server-table-unreadable")
    elif head_bound != server_head_bound:
        problems.append(
            "classification:head-bound-drift:"
            f"contract-only={sorted(head_bound - server_head_bound)}:"
            f"server-only={sorted(server_head_bound - head_bound)}"
        )
    classified = [tag for tags in lists.values() for tag in tags]
    if len(classified) != len(set(classified)):
        problems.append("classification:duplicate-tag")
    sentence = re.search(
        r"reserved tags\s+\((\d+), (\d+), (\d+), (\d+), (\d+), (\d+), (\d+)\)", spec
    )
    stated_reserved = {int(tag) for tag in sentence.groups()} if sentence else set()
    if stated_reserved != reserved:
        problems.append(
            f"classification:reserved:{sorted(stated_reserved)}!={sorted(reserved)}"
        )
    # The complete version 1 closed partition, then the version 2 partition
    # obtained by adding 306/307 to head-bound only. No subset comparison:
    # every tag on each side must match exactly.
    v1_method_tags = {tag_of[name] for name in all_names} if all_names else set(tag_of.values())
    # Only the v1-table members of the code reserved set belong in the v1
    # partition: the fresh v3-only rows (605-607) are reserved outside
    # version 3 but were never v1 tags, so they stay out of this side
    # exactly like the prose sentence keeps naming the whole code set.
    reserved_v1 = reserved & v1_method_tags
    covered_v1 = set(classified) | {SESSION_OPEN_TAG} | reserved_v1
    if tag_of and all_names and covered_v1 != v1_method_tags:
        problems.append(
            "classification:partition-v1:"
            f"unclassified={sorted(v1_method_tags - covered_v1)}:"
            f"unknown={sorted(covered_v1 - v1_method_tags)}"
        )
    extension = spec_paragraph(spec, "Protocol version 2 extension")
    extension_pairs = re.findall(r"`([a-z_.]+)`\s+\((\d+)\)", extension)
    extension_tags = {int(tag) for _, tag in extension_pairs}
    if extension_tags != {306, 307}:
        problems.append(f"classification:extension-tags:{sorted(extension_tags)}")
    for name, tag in extension_pairs:
        if smp1_rows.get(tag) != name:
            problems.append(f"classification:extension-smp1-name:{name}:{tag}")
    if tag_of.get("EntityVersion") != 306:
        problems.append(
            f"classification:entity-variant-tags:EntityVersion:{tag_of.get('EntityVersion')}"
        )
    if tag_of.get("EntitySignature") != 307:
        problems.append(
            f"classification:entity-variant-tags:EntitySignature:{tag_of.get('EntitySignature')}"
        )
    if head_bound | {306, 307} != server_head_bound_versioned:
        problems.append(
            "classification:head-bound-versioned-drift:"
            f"contract-v2={sorted((head_bound | {306, 307}) - server_head_bound_versioned)}:"
            f"server-only={sorted(server_head_bound_versioned - head_bound - {306, 307})}"
        )
    if versioned_additions != {306, 307}:
        problems.append(f"classification:versioned-additions:{sorted(versioned_additions)}")
    return lists


def check_current_delta_review(
    section: dict, expected_revision: int, status: object, problems: list[str]
) -> None:
    """The revision-bound current review record for this contract delta.

    Historical review fields keep their own revisions and never satisfy the
    current delta: only this object, bound to the anchored Status revision,
    admits freeze/complete, while draft/review-pending states stay valid
    with PENDING.
    """
    review = section.get("current_delta_review")
    if not isinstance(review, dict) or set(review) != {
        "contract_revision",
        "ariadne",
        "nabu",
        "vulcan",
    }:
        problems.append("review:current-delta-shape")
        return
    revision = review.get("contract_revision")
    if type(revision) is not int or revision != expected_revision:
        problems.append(f"review:current-delta-revision:{revision!r}")
    for lane in ("ariadne", "nabu", "vulcan"):
        if review.get(lane) not in (
            "PENDING",
            "PASS",
            "NEEDS_WORK",
            "FAIL",
            "INCOMPLETE",
        ):
            problems.append(f"review:current-delta-judgment:{lane}")
    if status in (FROZEN_STATUS, COMPLETE_STATUS) or (
        isinstance(status, str) and "CONTRACT_FROZEN" in status
    ):
        if not all(review.get(lane) == "PASS" for lane in ("ariadne", "nabu", "vulcan")):
            problems.append("review:current-delta-frozen-requires-pass")


def main() -> int:
    problems: list[str] = []
    for path in (
        SPEC,
        ADR,
        WORK_PACKAGES,
        SUMMARY,
        ERROR_CODES,
        ID_CRATE,
        SMP1_SPEC,
        CAPSULE_SPEC,
        SERVER_MODULE,
        REGISTRY_MODULE,
        CAPSULE_MODULE,
    ):
        if not path.exists():
            problems.append(f"missing:{path.relative_to(ROOT)}")
    if problems:
        print(json.dumps({"problems": problems, "result": "FAIL"}, indent=2))
        return 1

    spec = read(SPEC)
    for marker in SPEC_MARKERS:
        if marker not in spec:
            problems.append(f"spec-marker:{marker}")
    for numeric, symbol in CODES:
        if f"| {numeric} | `{symbol}` |" not in spec:
            problems.append(f"spec-code:{symbol}")
    revision = re.search(r"^Status:.*revision (\d+)", spec, flags=re.M)
    if revision is None or int(revision.group(1)) != CONTRACT_REVISION:
        problems.append("spec-revision")
    smp1 = read(SMP1_SPEC)
    smp1_revision = re.search(
        r"^Status: S20-400 contract draft, revision (\d+)", smp1, flags=re.M
    )
    if smp1_revision is None or int(smp1_revision.group(1)) != SMP1_REVISION:
        problems.append("smp1-revision-pin")
    capsule_spec = read(CAPSULE_SPEC)
    capsule_revision = re.search(
        r"^Status: S20-320 full contract draft, revision (\d+)",
        capsule_spec,
        flags=re.M,
    )
    if capsule_revision is None or int(capsule_revision.group(1)) != CAPSULE_REVISION:
        problems.append("capsule-revision-pin")
    adr = read(ADR)
    for marker in ADR_MARKERS:
        if marker not in adr:
            problems.append(f"adr-marker:{marker}")
    if f"revision {CONTRACT_REVISION}" not in adr:
        problems.append("adr-revision")
    if f"SMP1 revision {SMP1_REVISION}" not in re.sub(r"\s+", " ", adr):
        problems.append("adr-smp1-pin")
    packages = read(WORK_PACKAGES)
    for marker in WORK_PACKAGE_MARKERS:
        if marker not in packages:
            problems.append(f"work-package-marker:{marker}")
    if (
        f"contract draft revision {CONTRACT_REVISION} ("
        not in packages.split("| S20-330 |")[-1].split("\n")[0]
    ):
        problems.append("work-package-revision")
    codes = read(ERROR_CODES)
    if "33000 through 33007" not in codes:
        problems.append("error-codes:range-sentence")
    else:
        paragraph = codes.split("33000 through 33007")[1].split("S20-350")[0]
        if "reserves, rather than freezes, these codes" in paragraph:
            problems.append("error-codes:not-frozen")
        if f"contract draft revision {CONTRACT_REVISION}" not in paragraph:
            problems.append("error-codes:revision")
    for numeric, symbol in CODES:
        if f"| {numeric} | `{symbol}` |" not in codes:
            problems.append(f"error-codes:row:{symbol}")
    module = read(SESSION_MODULE) if SESSION_MODULE.exists() else ""
    server_tests = read(SERVER_TESTS) if SERVER_TESTS.exists() else ""
    for threat in THREAT_MATRICES:
        matrix_path = EVIDENCE / threat / "matrix.json"
        if not matrix_path.is_file():
            problems.append(f"threat-evidence:missing:{threat}")
            continue
        try:
            matrix = json.loads(matrix_path.read_text(encoding="utf-8"))
        except json.JSONDecodeError:
            problems.append(f"threat-evidence:unparsable:{threat}")
            continue
        if matrix.get("result") != "PASS" or not matrix.get("tests"):
            problems.append(f"threat-evidence:fail:{threat}")
        # A matrix names tests that exist: a renamed or deleted test would
        # otherwise leave the evidence pointing at nothing.
        for test in matrix.get("tests", []):
            if f"fn {test}(" not in module and f"fn {test}(" not in server_tests:
                problems.append(f"threat-evidence:test-missing:{threat}:{test}")

    summary = json.loads(read(SUMMARY))
    section = summary.get("session_handle_profile")
    if not isinstance(section, dict):
        problems.append("machine-summary:session_handle_profile missing")
        section = {}
    status = section.get("status")
    expected = {
        "contract": "docs/spec/SESSION_HANDLE_PROFILE_V1.md",
        "contract_revision": CONTRACT_REVISION,
        "adr": "docs/adr/ADR-0033-negotiated-session-boundary.md",
        "identifier_domain": "sley2.session.v1",
        "new_stable_error_codes": len(CODES),
        "handles": "POSITIONAL_WITH_EXPECTED_ROOT",
        "implementation_complete": status == COMPLETE_STATUS,
    }
    for key, value in expected.items():
        if section.get(key) != value:
            problems.append(f"machine-summary:{key}")
    if status not in (DRAFT_STATUS, FROZEN_STATUS) + IMPLEMENTATION_STATUSES:
        problems.append("machine-summary:status")
    check_current_delta_review(section, CONTRACT_REVISION, status, problems)
    for list_key in OPEN_LISTS:
        if list_key in section or f"{list_key}_count" in section:
            items = section.get(list_key, [])
            if not isinstance(items, list) or section.get(f"{list_key}_count") != len(
                items
            ):
                problems.append(f"machine-summary:{list_key}_count")

    present = []
    if SESSION_MODULE.exists():
        present.append("crates/sley-protocol/src/session.rs")
    identifiers = read(ID_CRATE)
    if ID_MARKERS[0] in identifiers:
        present.append("sley-id:Session")
    # A draft still awaiting review must have no implementation yet; a
    # frozen reviewed document coexists with its implementation normally.
    if status == DRAFT_STATUS and present:
        problems.append(f"implementation-before-stage:{present}")
    if status in (FROZEN_STATUS,) + IMPLEMENTATION_STATUSES:
        for marker in SESSION_MARKERS:
            if marker not in module:
                problems.append(f"session-marker:{marker}")
        server = read(SERVER_MODULE)
        for marker in SERVER_MARKERS:
            if marker not in server:
                problems.append(f"server-marker:{marker}")
        registry = read(REGISTRY_MODULE)
        for marker in REGISTRY_MARKERS:
            if marker not in registry:
                problems.append(f"registry-marker:{marker}")
        capsule = read(CAPSULE_MODULE)
        for marker in CAPSULE_MARKERS:
            if marker not in capsule:
                problems.append(f"capsule-marker:{marker}")
        check_method_classification(spec, server, registry, smp1, problems)
        # Each variant is bound to its exact symbol and numeric pair, so two
        # swapped numerics fail even though every symbol and every literal
        # is still present.
        symbols = dict(
            re.findall(
                r'Self::(\w+) => "(SESSION_\w+)"',
                rust_block(module, "pub const fn as_str(self) -> &'static str {"),
            )
        )
        numerics = {
            name: int(value.replace("_", ""))
            for name, value in re.findall(
                r"Self::(\w+) => (33_\d{3}),",
                rust_block(module, "pub const fn numeric(self) -> u32 {"),
            )
        }
        pairs = {(numerics.get(name), symbol) for name, symbol in symbols.items()}
        if len(symbols) != len(CODES) or len(numerics) != len(CODES):
            problems.append("module-codes:variant-count")
        for pair in set(CODES) - pairs:
            problems.append(f"module-code-pair:{pair[1]}:{pair[0]}")
        for marker in ID_MARKERS:
            if marker not in identifiers:
                problems.append(f"id-marker:{marker}")
        if status == REVIEW_PENDING_STATUS:
            for key, pass_key in LANE_PASS_FIELD.items():
                if not str(section.get(key, "")).startswith("FAIL"):
                    continue
                lane_items = [
                    item
                    for list_key in OPEN_LISTS
                    for item in section.get(list_key, [])
                    if str(item).startswith(LANE_PREFIX[key])
                ]
                superseded = str(section.get(pass_key, "")).startswith("PASS")
                if not lane_items and not superseded:
                    problems.append(f"review-without-lane-items:{key}")

    result = {
        "contract": "s20-330-session-handle-profile-v1",
        "status": status,
        "revision": int(revision.group(1)) if revision else None,
        "smp1_revision": SMP1_REVISION,
        "capsule_revision": CAPSULE_REVISION,
        "implementation_present": present,
        "new_stable_error_codes": len(CODES),
        "problems": problems,
        "result": "PASS" if not problems else "FAIL",
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0 if not problems else 1


if __name__ == "__main__":
    raise SystemExit(main())
