#!/usr/bin/env python3
"""Check the S20-400 SMP1 protocol contract and its stage."""

from __future__ import annotations

import json
import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SPEC = ROOT / "docs/spec/SMP1.md"
ADR = ROOT / "docs/adr/ADR-0032-smp1-transport-boundary.md"
WORK_PACKAGES = ROOT / "docs/WORK_PACKAGES.md"
SUMMARY = ROOT / "machineresearch/sley-2.0/machine-summary.json"
ERROR_CODES = ROOT / "docs/spec/ERROR_CODES_V1.md"
CRATE = ROOT / "crates/sley-protocol"

DRAFT_STATUS = "S20_400_CONTRACT_DRAFT_REVIEW_PENDING"
DRAFT_IN_PROGRESS_STATUS = "S20_400_CONTRACT_DRAFT_S20_410_IN_PROGRESS"
FROZEN_STATUS = "S20_400_CONTRACT_FROZEN"
IMPLEMENTED_STATUS = "S20_400_CONTRACT_DRAFT_S20_410_IMPLEMENTED_REVIEW_PENDING"
COMPLETE_STATUS = "S20_400_COMPLETE"
IMPLEMENTATION_STATUSES = (DRAFT_IN_PROGRESS_STATUS, IMPLEMENTED_STATUS, COMPLETE_STATUS)

CODES = (
    (40000, "PROTOCOL_VERSION_UNSUPPORTED"),
    (40001, "PROTOCOL_FRAME_INVALID"),
    (40002, "PROTOCOL_FRAME_TOO_LARGE"),
    (40003, "PROTOCOL_NO_COMMON_PROFILE"),
    (40004, "PROTOCOL_DOWNGRADE"),
    (40005, "PROTOCOL_REQUEST_ID_CONFLICT"),
    (40006, "PROTOCOL_SESSION_CLOSED"),
    (40007, "PROTOCOL_METHOD_UNSUPPORTED"),
    (40008, "PROTOCOL_PAYLOAD_INVALID"),
    (40009, "PROTOCOL_LIMIT_EXCEEDED"),
    (40010, "PROTOCOL_CANCELLED"),
    (40011, "PROTOCOL_INTERNAL_INVARIANT"),
)
METHOD_TAGS = (
    [100, 101, 102, 103, 104]
    + list(range(200, 215))
    + [300, 301, 302, 303, 304, 305]
    + [400, 401, 402, 403, 404]
    + [500, 501, 502, 503, 504]
    + [600, 601, 602, 603, 604]
)
# Protocol version 1 is the frozen table above. Protocol version 2 is the
# sorted union with exactly the two S20-310 entity-read additions; no
# second independently maintained 43-row table exists.
V2_ADDITIONS = (306, 307)
V2_METHOD_TAGS = tuple(sorted(METHOD_TAGS + list(V2_ADDITIONS)))
CONTRACT_REVISION = 15
# Revision 13/14 normative text for method 201 (anchored on whitespace-
# flattened text, so reverting any of it fails the gate).
WORKSPACE_OPEN_ANCHORS = (
    ("v1-row-201", "| 201 | `workspace.open` | none | accepted head summary (appendix A) | S20-390 |"),
    ("appendix-a-row-201-request",
     "| 201 `workspace.open` | empty; a non-empty body is `PROTOCOL_PAYLOAD_INVALID` under every version (revision 13) |"),
    ("appendix-a-row-201-response",
     "`open_summary` under version 2 and every later selection whose table carries row 201 (version 3)"),
    ("open-summary-grammar", "7: uvar(tombstones), 8: ReceiptId, [9: IndexSnapshotId])"),
    ("optional-by-omission",
     "An optional record field written `[n: T]` is optional by omission: when absent the field is not encoded at all"),
    ("open-summary-scope",
     "`open_summary` (revisions 13 to 15) is the `workspace.open` response under version 2 and under every later "
     "selection whose method table includes version 2's row 201"),
    ("field-9-pointer", "Field 9 is a pointer, not evidence"),
    ("entrypoints-admit-version-3",
     "`ProtocolFrame::validate_for_version`) admit only selections 1, 2, and 3, like `negotiate_versioned`"),
    ("version-1-compat",
     "with two version 1 observable changes on record: a non-empty 201 body, previously ignored"),
    ("version-1-native-filter-compat",
     "a version 1 selection drops the native tags 605, 606, and 607 from the intersection as well as 306 and 307"),
    ("per-selection-filter",
     "under selected version 1 it removes the version-2 tags 306 and 307 and the native tags 605, 606, and 607; "
     "under selected version 2 it removes 605, 606, and 607; under selected version 3 without the native-tests "
     "feature bit it removes 601, 602, 605, 606, and 607"),
)
NATIVE_SPEC = ROOT / "docs/spec/NATIVE_TEST_ADMISSION_V1.md"


def workspace_open_anchor_problems(spec: str, native: str) -> list[str]:
    """Anchors for the method 201 text and the version 3 owner's pin."""
    flat = re.sub(r"\s+", " ", spec)
    problems = [f"spec-anchor:{name}" for name, text in WORKSPACE_OPEN_ANCHORS if text not in flat]
    native_flat = re.sub(r"\s+", " ", native)
    pins = re.findall(r"`docs/spec/SMP1\.md` at revision (\d+)", native_flat)
    if pins != [str(CONTRACT_REVISION)]:
        problems.append(f"v3-owner-pin:{pins}")
    return problems
V1_SECTION = "### Protocol version 1"
V2_SECTION = "### Protocol version 2 additions"
V2_SECTION_END = "## 5. Bounded context"
APPENDIX_A = "## Appendix A."
APPENDIX_B = "## Appendix B."
APPENDIX_C = "## Appendix C."
APPENDIX_D = "## Appendix D."
SPEC_MARKERS = (
    "Retryability is an explicit mapping from the owner's symbol",
    "`AFTER_REQUERY` names exactly `REF_CAS_STALE`,",
    "`AFTER_LIMIT_CHANGE` names exactly",
    "`AFTER_CAPABILITY` names exactly",
    "# Sley Machine Protocol v1 (SMP1)",
    "Status: S20-400 contract draft",
    "## 1. Framing",
    "digest domain  = sley2.protocol-frame.v1 -> ProtocolFrameId",
    "frame = u64be(frame_length) || protocol_envelope",
    "never a frame defect",
    "## 2. Handshake",
    "`PROTOCOL_DOWNGRADE` (threat T45)",
    "The codec applies the same split at",
    "the intersection must contain `session.open` (100)",
    "A method family is the hundred-group of a tag",
    "## 3. Sessions and request identity",
    "`PROTOCOL_REQUEST_ID_CONFLICT` (threat T46)",
    "identifier 0 never enters a session",
    "carries identifier 0, no other",
    "## 4. Method families and tags",
    "### Protocol version 1",
    "### Protocol version 2 additions",
    "`entity.version`",
    "`entity.signature`",
    "43 rows total, 39 dispatched",
    "A tag added after freeze takes a new",
    "`SMP1-RESERVED-S20-370`",
    "`SMP1-RESERVED-S20-620`",
    "## 5. Bounded context",
    "Zero counts on a failure response or an event frame mean",
    "## 6. Failure envelope",
    "transport-supplied reason bytes from the set below, else empty",
    "carries it, and the terminal frame of a failed stream carries it",
    "`details` are transport-supplied reason bytes",
    "The envelope's `incident` is none at this revision",
    "## 7. Cancellation and streaming",
    "dispatch costs one unit up front",
    "## 8. JSON bridge",
    "## 11. Explicit exclusions",
    "cancellation (`PROTOCOL_CANCELLED`, skipping binding and budget)",
    "## 10. Required evidence",
    "same-lane `PASS` obligations or itemized",
    "## Appendix A. Body records of the dispatched methods (S20-410)",
    "## Appendix D.",
    "no non-reserved method answers a deferred detail",
    "## Appendix B. Cancellation, streaming, and budget records (S20-440)",
    "stream_chunk = record(1: uvar(index), 2: uvar(total), 3: bytes(chunk))",
    "## Appendix C. Body records of the slice C methods (S20-410 slice C, revision 7; profile selector revision 8)",
    "6: uvar(profile: 1 restricted_v1 | 2 extended_v1))",
    "**The server owns the retention snapshot.**",
    "**Execute is head-bound.**",
    "needs the negotiated `extended_execute` feature bit",
    "execution_report = record(1: ExecutionReportId[32], 2: bytes(execution report preimage, `SLEYEXR1`))",
    "cancel latency bound is therefore exactly one request execution",
)
ADR_MARKERS = (
    "# ADR-0032: SMP1 transport, negotiation, and identity-scoping boundary",
    "1. **Transport owns no semantics.**",
    "2. **Derived negotiation.**",
    "3. **Session-scoped strictly increasing request identifiers.**",
    "4. **Frozen method table.**",
    "5. **Bounded context on every response.**",
    "6. **Frames are SCB1 envelopes.**",
    "under a single contract tag 400",
    "7. **Staging.**",
    "8. **Batch admission with cancellation before execution.**",
    "9. **Transcript-bound identity.**",
)
ERROR_CODE_ROWS = (
    (40000, "PROTOCOL_VERSION_UNSUPPORTED"),
    (40001, "PROTOCOL_FRAME_INVALID"),
    (40002, "PROTOCOL_FRAME_TOO_LARGE"),
    (40003, "PROTOCOL_NO_COMMON_PROFILE"),
    (40004, "PROTOCOL_DOWNGRADE"),
    (40005, "PROTOCOL_REQUEST_ID_CONFLICT"),
    (40006, "PROTOCOL_SESSION_CLOSED"),
    (40007, "PROTOCOL_METHOD_UNSUPPORTED"),
    (40008, "PROTOCOL_PAYLOAD_INVALID"),
    (40009, "PROTOCOL_LIMIT_EXCEEDED"),
    (40010, "PROTOCOL_CANCELLED"),
    (40011, "PROTOCOL_INTERNAL_INVARIANT"),
)
RESERVED_TAGS = (305, 503, 601, 602)
STALE_TOKEN = "S20-410-SLICE-C-DEFERRED"
WORK_PACKAGE_MARKERS = ("`docs/spec/SMP1.md`", "ADR-0032")


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


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
    for path in (SPEC, ADR, WORK_PACKAGES, SUMMARY, ERROR_CODES):
        if not path.exists():
            problems.append(f"missing:{path.relative_to(ROOT)}")
    if problems:
        print(json.dumps({"problems": problems, "result": "FAIL"}, indent=2))
        return 1

    spec = read(SPEC)
    for marker in SPEC_MARKERS:
        if marker not in spec:
            problems.append(f"spec-marker:{marker}")
    if STALE_TOKEN in spec:
        problems.append(f"spec-stale-token:{STALE_TOKEN}")
    for numeric, symbol in CODES:
        if f"| {numeric} | `{symbol}` |" not in spec:
            problems.append(f"spec-code:{symbol}")
    lines = spec.splitlines()
    for heading, expected_count in (
        (V1_SECTION, 1),
        (V2_SECTION, 1),
        (V2_SECTION_END, 1),
        (APPENDIX_A, 1),
        (APPENDIX_B, 1),
        (APPENDIX_C, 1),
        (APPENDIX_D, 1),
    ):
        found = sum(1 for line in lines if line == heading or line.startswith(heading))
        if found != expected_count:
            problems.append(f"spec-section:{heading}:{found}")
    v1_at = lines.index(V1_SECTION) if V1_SECTION in lines else -1
    v2_at = lines.index(V2_SECTION) if V2_SECTION in lines else -1
    v2_end = lines.index(V2_SECTION_END) if V2_SECTION_END in lines else -1
    if not 0 <= v1_at < v2_at < v2_end:
        problems.append("spec-section-order")
        v1_tags: list[int] = []
        v2_tags: list[int] = []
    else:
        v1_tags = [
            int(tag)
            for line in lines[v1_at + 1 : v2_at]
            for tag in re.findall(r"^\| (\d{3}) \| `[a-z._]+` \|", line)
        ]
        v2_tags = [
            int(tag)
            for line in lines[v2_at + 1 : v2_end]
            for tag in re.findall(r"^\| (\d{3}) \| `[a-z._]+` \|", line)
        ]
        # A method row outside the declared table sections is drift, never
        # silently absorbed into either table.
        scoped = len(v1_tags) + len(v2_tags)
        total = len(re.findall(r"^\| (\d{3}) \| `[a-z._]+` \|", spec, flags=re.M))
        if total != scoped:
            problems.append(f"spec-method-table-scope:total={total}:scoped={scoped}")
    if v1_tags != list(METHOD_TAGS):
        problems.append(f"spec-method-table-v1:{v1_tags}")
    if v2_tags != list(V2_ADDITIONS):
        problems.append(f"spec-method-table-v2:{v2_tags}")
    if sorted(v1_tags + v2_tags) != list(V2_METHOD_TAGS):
        problems.append("spec-method-table-union")
    # Every non-reserved version-1 tag has a legacy appendix body row in
    # appendix A or C; the version-2 additions are referenced in appendix D
    # only. Reserved tags have no body row anywhere. Every body row in the
    # document lives in exactly one of those three regions: a row anywhere
    # else is drift, even when the scoped regions still match.
    def appendix_region(start: str, end: str) -> str:
        if start not in spec or end not in spec:
            return ""
        return spec.split(start)[1].split(end)[0]

    body_row = re.compile(r"^\| (\d{3}) `[^`]+` \|", flags=re.M)
    region_text = (
        appendix_region(APPENDIX_A, APPENDIX_B)
        + appendix_region(APPENDIX_C, APPENDIX_D)
    )
    d_text = spec.split(APPENDIX_D)[1] if spec.count(APPENDIX_D) == 1 else ""
    region_tags = [int(tag) for tag in body_row.findall(region_text)]
    appendix_d_tags = sorted(int(tag) for tag in body_row.findall(d_text))
    all_body_tags = [int(tag) for tag in body_row.findall(spec)]
    if len(all_body_tags) != len(region_tags) + len(appendix_d_tags):
        problems.append(
            "spec-appendix-outside-region:"
            f"total={len(all_body_tags)}:"
            f"scoped={len(region_tags) + len(appendix_d_tags)}"
        )
    if len(all_body_tags) != len(set(all_body_tags)):
        seen: set[int] = set()
        doubled: list[int] = []
        for tag in all_body_tags:
            if tag in seen and tag not in doubled:
                doubled.append(tag)
            seen.add(tag)
        problems.append(f"spec-appendix-duplicate:{sorted(doubled)}")
    for tag in RESERVED_TAGS:
        if tag in all_body_tags:
            problems.append(f"spec-appendix-reserved:{tag}")
    appendix_tags = sorted(region_tags)
    live_tags = sorted(tag for tag in METHOD_TAGS if tag not in RESERVED_TAGS)
    if appendix_tags != live_tags:
        problems.append(f"spec-appendix-coverage:{appendix_tags}")
    if appendix_d_tags != sorted(V2_ADDITIONS):
        problems.append(f"spec-appendix-d:{appendix_d_tags}")
    adr = read(ADR)
    for marker in ADR_MARKERS:
        if marker not in adr:
            problems.append(f"adr-marker:{marker}")
    problems.extend(workspace_open_anchor_problems(spec, read(NATIVE_SPEC)))
    if f"revision {CONTRACT_REVISION}" not in adr:
        problems.append(f"adr-revision:{CONTRACT_REVISION}")
    packages = read(WORK_PACKAGES)
    for marker in WORK_PACKAGE_MARKERS:
        if marker not in packages:
            problems.append(f"work-package-marker:{marker}")
    codes_text = read(ERROR_CODES)
    if "40000 through 40011" not in codes_text:
        problems.append("error-codes:range-sentence")
    for numeric, symbol in ERROR_CODE_ROWS:
        if f"| {numeric} | `{symbol}` |" not in codes_text:
            problems.append(f"error-codes:row:{symbol}")
    if "carries numeric 36002" not in codes_text:
        problems.append("error-codes:stale-root-alias")
    # The retryability enumeration in section 6 and the server lists agree
    # symbol for symbol in both directions: a list the contract omits and
    # a symbol the implementation omits both fail the gate.
    server = read(ROOT / "crates/sley-protocol/src/server.rs")
    AFTER = ("AFTER_REQUERY", "AFTER_LIMIT_CHANGE", "AFTER_CAPABILITY")
    starts = {
        name: f"`{name}` names exactly" for name in AFTER
    }
    ends = {
        "AFTER_REQUERY": "`AFTER_LIMIT_CHANGE` names exactly",
        "AFTER_LIMIT_CHANGE": "`AFTER_CAPABILITY` names exactly",
        "AFTER_CAPABILITY": "Every other symbol is",
    }
    arrays = {
        "AFTER_REQUERY": "RETRY_AFTER_REQUERY",
        "AFTER_LIMIT_CHANGE": "RETRY_AFTER_LIMIT_CHANGE",
    }
    for name in AFTER:
        span = spec.split(starts[name])[1].split(ends[name])[0]
        contracted = set(re.findall(r"`([A-Z][A-Z0-9_]*_[A-Z0-9_]+)`", span)) - set(AFTER)
        if name in arrays:
            listed = re.findall(
                r'"([A-Z][A-Z0-9_]+)"', server.split(arrays[name])[1].split("];")[0]
            )
            if contracted != set(listed):
                problems.append(f"retryability-map:{name}:{sorted(contracted ^ set(listed))}")
        else:
            if contracted != {"PROTOCOL_METHOD_UNSUPPORTED"}:
                problems.append(f"retryability-map:{name}:{sorted(contracted)}")
            if "failure.retryability = Retryability::AfterCapability" not in server:
                problems.append("retryability-map:reserved-capability")
    if "FEATURE_EXTENDED_EXECUTE" not in server:
        problems.append("execute-profile:feature-gate")

    summary = json.loads(read(SUMMARY))
    section = summary.get("protocol")
    if not isinstance(section, dict):
        problems.append("machine-summary:protocol missing")
        section = {}
    status = section.get("status")
    # Reverse pins: exactly one line-anchored Current composition record
    # names the composing contracts' current revisions, and each pin inside
    # that record equals its own status line (the forward pins live in
    # check_smp1_json_bridge_contract.py and check_cli_contract.py).
    # Historical closeout sentences keep their own revisions and never
    # satisfy these pins.
    composition_hits = list(
        re.finditer(r"^Current composition \(revision (\d+)\):", spec, flags=re.M)
    )
    if len(composition_hits) != 1 or int(composition_hits[0].group(1)) != CONTRACT_REVISION:
        problems.append(
            f"spec-current-composition:{[hit.group(1) for hit in composition_hits]}"
        )
        composition_text = ""
    else:
        start = composition_hits[0].start()
        end = spec.find("\n\n", start)
        composition_text = spec[start:end] if end > start else spec[start:]
    for name, path, status_re, pin_re in (
        ("bridge", ROOT / "docs/spec/SMP1_JSON_BRIDGE_V1.md", r"^Status: S20-420 contract draft, revision (\d+)", r"`docs/spec/SMP1_JSON_BRIDGE_V1\.md` revision (\d+)"),
        ("cli", ROOT / "docs/spec/SLEY_CLI_V1.md", r"^Status: S20-430 contract draft, revision (\d+)", r"`docs/spec/SLEY_CLI_V1\.md` revision (\d+)"),
    ):
        found = re.search(status_re, path.read_text(encoding="utf-8"), flags=re.M)
        pins = re.findall(pin_re, composition_text) if composition_text else []
        if found is None:
            problems.append(f"reverse-pin:{name}:status-line")
        elif len(pins) != 1 or pins[0] != found.group(1):
            problems.append(f"reverse-pin:{name}:revision-{found.group(1) if found else '?'}")
    status_hits = re.findall(
        r"^Status: S20-400 contract draft, revision (\d+)", spec, flags=re.M
    )
    if len(status_hits) != 1:
        problems.append("spec-revision:status-line")
        contract_revision = None
    else:
        contract_revision = int(status_hits[0])
        if contract_revision != CONTRACT_REVISION:
            problems.append("spec-revision:status-line")
    expected = {
        "contract": "docs/spec/SMP1.md",
        "adr": "docs/adr/ADR-0032-smp1-transport-boundary.md",
        "method_count": len(METHOD_TAGS),
        "new_stable_error_codes": len(CODES),
        "frame_domain": "sley2.protocol-frame.v1",
        "handshake_domain": "sley2.protocol-handshake.v1",
        "contract_revision": contract_revision,
        "dispatched_methods": [t for t in METHOD_TAGS if t not in RESERVED_TAGS],
        "frame_contract_tags": [400],
        "contract_complete": status in (FROZEN_STATUS, COMPLETE_STATUS),
        "version_2": {
            "method_count": len(V2_METHOD_TAGS),
            "dispatched_methods": [t for t in V2_METHOD_TAGS if t not in RESERVED_TAGS],
            "reserved_methods": list(RESERVED_TAGS),
        },
    }
    for key, value in expected.items():
        if section.get(key) != value:
            problems.append(f"machine-summary:{key}")
    if status not in (DRAFT_STATUS, FROZEN_STATUS) + IMPLEMENTATION_STATUSES:
        problems.append("machine-summary:status")
    if contract_revision is not None:
        check_current_delta_review(section, contract_revision, status, problems)

    present = []
    if CRATE.exists():
        present.append("crates/sley-protocol")
    # Contract acceptance (section 10) is the freeze: the revision-bound
    # current review admits it, and an implementation coexisting with a
    # frozen reviewed document is normal (S20-410 implements under the
    # draft). Only a draft still awaiting review must have no
    # implementation yet.
    if status == DRAFT_STATUS and present:
        problems.append(f"implementation-before-stage:{present}")
    # Implemented under a draft means tracked, never silently pending: a
    # FAIL round must be itemized in non-empty same-lane register-first
    # open lists, or superseded by a same-lane PASS obligation. Unrelated
    # lanes' items never satisfy a lane, and an empty list set with no
    # same-lane PASS fails the gate.
    lane_pass_field = {
        "ariadne_contract_review": "ariadne_review",
        "nabu_architecture_review": "nabu_review",
        "vulcan_surface_review": "vulcan_review",
    }
    lane_prefix = {
        "ariadne_contract_review": "Ariadne ",
        "nabu_architecture_review": "Nabu ",
        "vulcan_surface_review": "Vulcan ",
    }
    open_lists = ("p1_open", "p2_open", "p3_open")
    if status == IMPLEMENTED_STATUS:
        for key in ("ariadne_contract_review", "nabu_architecture_review", "vulcan_surface_review"):
            if str(section.get(key, "")).startswith("FAIL"):
                items = [
                    item
                    for list_key in open_lists
                    for item in section.get(list_key, [])
                ]
                lane_items = [
                    item for item in items if item.startswith(lane_prefix[key])
                ]
                superseded = str(
                    section.get(lane_pass_field[key], "")
                ).startswith("PASS")
                if not lane_items and not superseded:
                    problems.append(f"review-without-lane-items:{key}")

    revision = re.search(r"revision (\d+)", spec)
    result = {
        "contract": "s20-400-smp1-v1",
        "status": status,
        "revision": contract_revision,
        "method_count": len(METHOD_TAGS),
        "implementation_present": present,
        "new_stable_error_codes": len(CODES),
        "problems": problems,
        "result": "PASS" if not problems else "FAIL",
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0 if not problems else 1


if __name__ == "__main__":
    raise SystemExit(main())
