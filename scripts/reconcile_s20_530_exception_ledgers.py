#!/usr/bin/env python3
"""Independently reconcile serialized S20-530 v8 exception partitions."""

from __future__ import annotations

import hashlib
import json
import os
import stat
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_TEST_PLAN = (
    ROOT / "evidence/validation/s20-530-crash-recovery-test-plan-v1.json"
)
MAXIMUM_INPUT_BYTES = 16 * 1024 * 1024


class ReconciliationError(ValueError):
    """One serialized partition cannot be reconciled exactly."""


def canonical_json_sha256(value: object) -> str:
    payload = json.dumps(
        value,
        separators=(",", ":"),
        ensure_ascii=False,
        allow_nan=False,
    )
    return hashlib.sha256(payload.encode("utf-8")).hexdigest()


def canonical_leaf(record: dict[str, object]) -> dict[str, object]:
    result = dict(record)
    result["canonical_leaf_sha256"] = canonical_json_sha256(result)
    return result


def reject_constant(token: str) -> object:
    raise ReconciliationError(f"non-finite JSON constant is forbidden: {token}")


def exact_object(pairs: list[tuple[str, object]]) -> dict[str, object]:
    result: dict[str, object] = {}
    for key, value in pairs:
        if key in result:
            raise ReconciliationError(f"duplicate JSON key is forbidden: {key}")
        result[key] = value
    return result


def load_strict_json(path: Path) -> dict[str, object]:
    try:
        metadata = path.lstat()
    except OSError as error:
        raise ReconciliationError(f"cannot inspect {path}: {error}") from error
    if not stat.S_ISREG(metadata.st_mode):
        raise ReconciliationError(f"input is not a regular file: {path}")
    if metadata.st_size > MAXIMUM_INPUT_BYTES:
        raise ReconciliationError(f"input exceeds the byte limit: {path}")
    descriptor = os.open(path, os.O_RDONLY | os.O_CLOEXEC | os.O_NOFOLLOW)
    try:
        payload = os.read(descriptor, MAXIMUM_INPUT_BYTES + 1)
        if len(payload) > MAXIMUM_INPUT_BYTES:
            raise ReconciliationError(f"input exceeds the byte limit: {path}")
        if os.read(descriptor, 1):
            raise ReconciliationError(
                f"input changed or exceeds the byte limit: {path}"
            )
        after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    if (metadata.st_dev, metadata.st_ino, metadata.st_size) != (
        after.st_dev,
        after.st_ino,
        after.st_size,
    ):
        raise ReconciliationError(f"input changed while read: {path}")
    try:
        value = json.loads(
            payload.decode("utf-8", errors="strict"),
            object_pairs_hook=exact_object,
            parse_constant=reject_constant,
        )
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ReconciliationError(
            f"cannot parse strict JSON from {path}: {error}"
        ) from error
    if not isinstance(value, dict):
        raise ReconciliationError(f"input root is not an object: {path}")
    return value


def require_object(value: object, label: str) -> dict[str, object]:
    if not isinstance(value, dict):
        raise ReconciliationError(f"{label} is not an object")
    return value


def require_list(value: object, label: str) -> list[object]:
    if not isinstance(value, list):
        raise ReconciliationError(f"{label} is not an array")
    return value


def require_string(value: object, label: str) -> str:
    if not isinstance(value, str) or not value:
        raise ReconciliationError(f"{label} is not a nonempty string")
    return value


def split_item_key(value: object, label: str) -> tuple[str, str, str]:
    key = require_string(value, label)
    parts = key.split(":", 2)
    if len(parts) != 3 or any(not part for part in parts):
        raise ReconciliationError(f"{label} is not source:owner:function")
    return parts[0], parts[1], parts[2]


def entry_callsite_evidence(callsite: dict[str, object], index: int) -> list[str]:
    evidence: list[str] = []
    unresolved = callsite.get("unresolved")
    if isinstance(unresolved, str) and unresolved:
        evidence.append(unresolved)
    elif isinstance(unresolved, list):
        evidence.extend(item for item in unresolved if isinstance(item, str) and item)
    if index == 0:
        for field, marker, expected in (
            ("dominating_scopes", "PUBLIC_ROOT_DOMINATING_SCOPE", []),
            ("dominating_exits", "PUBLIC_ROOT_DOMINATING_EXIT", []),
            ("preceding_question_mark_count", "PUBLIC_ROOT_PRECEDING_QUESTION_MARK", 0),
            ("preceding_early_exit_tokens", "PUBLIC_ROOT_PRECEDING_EARLY_EXIT", []),
        ):
            if callsite.get(field) != expected:
                evidence.append(marker)
    return evidence


def entry_inventory(
    manifest: dict[str, object],
) -> tuple[list[dict[str, object]], list[dict[str, object]]]:
    inventory: list[dict[str, object]] = []
    exceptions: list[dict[str, object]] = []
    for event_ordinal, (event_id, raw_event) in enumerate(manifest.items(), start=1):
        event = require_object(raw_event, f"{event_id} entry event")
        parent_sha256 = canonical_json_sha256(event)
        call_paths = require_list(event.get("call_paths"), f"{event_id} call paths")
        reverse_callers = require_object(
            event.get("reverse_callers"), f"{event_id} reverse callers"
        )
        record_ordinal = 0
        for path_ordinal, raw_path in enumerate(call_paths, start=1):
            path = require_object(raw_path, f"{event_id} call path")
            items = require_list(path.get("items"), f"{event_id} path items")
            callsites = require_list(path.get("callsites"), f"{event_id} callsites")
            if len(items) != len(callsites) + 1:
                raise ReconciliationError(f"{event_id} path edge cardinality differs")
            for edge_ordinal, raw_callsite in enumerate(callsites, start=1):
                callsite = require_object(raw_callsite, f"{event_id} callsite")
                caller_item = require_object(
                    items[edge_ordinal - 1], f"{event_id} caller item"
                )
                callee_item = require_object(
                    items[edge_ordinal], f"{event_id} callee item"
                )
                caller = require_string(
                    caller_item.get("item_key"), f"{event_id} caller key"
                )
                callee = require_string(
                    callee_item.get("item_key"), f"{event_id} callee key"
                )
                source, owner, function = split_item_key(caller, f"{event_id} caller")
                record_ordinal += 1
                unresolved_sha256 = canonical_json_sha256(callsite)
                structural = canonical_leaf(
                    {
                        "layer": "ENTRY_PATH",
                        "leaf_kind": "ENTRY_CALL_EDGE",
                        "event_id": event_id,
                        "event_ordinal": event_ordinal,
                        "record_ordinal": record_ordinal,
                        "source": source,
                        "owner": owner,
                        "function": function,
                        "path_ordinal": path_ordinal,
                        "edge_ordinal": edge_ordinal,
                        "caller": caller,
                        "callee": callee,
                        "parent_manifest_sha256": parent_sha256,
                        "unresolved_record_sha256": unresolved_sha256,
                    }
                )
                inventory.append(structural)
                evidence = entry_callsite_evidence(callsite, edge_ordinal - 1)
                if evidence:
                    exceptions.append(
                        {
                            "generated_digest_field": "generated_edge_sha256",
                            "generated_digest": structural["canonical_leaf_sha256"],
                            "event_id": event_id,
                            "event_ordinal": event_ordinal,
                            "record_ordinal": record_ordinal,
                            "source": source,
                            "owner": owner,
                            "function": function,
                            "parent_manifest_sha256": parent_sha256,
                            "unresolved_record_sha256": unresolved_sha256,
                            "path_ordinal": path_ordinal,
                            "edge_ordinal": edge_ordinal,
                            "caller": caller,
                            "callee": callee,
                            "unresolved_evidence": evidence,
                            "signature_sha256": caller_item.get("signature_sha256"),
                            "attribute_chain_sha256": caller_item.get(
                                "attribute_chain_sha256"
                            ),
                            "body_sha256": caller_item.get("body_sha256"),
                        }
                    )
        seen_targets: set[str] = set()
        for target, raw_reverse in reverse_callers.items():
            if target in seen_targets:
                continue
            seen_targets.add(target)
            reverse = require_object(raw_reverse, f"{event_id} reverse caller")
            source, owner, function = split_item_key(target, f"{event_id} target")
            record_ordinal += 1
            unresolved_sha256 = canonical_json_sha256(reverse)
            structural = canonical_leaf(
                {
                    "layer": "ENTRY_PATH",
                    "leaf_kind": "REVERSE_CALLER_INVENTORY",
                    "event_id": event_id,
                    "event_ordinal": event_ordinal,
                    "record_ordinal": record_ordinal,
                    "source": source,
                    "owner": owner,
                    "function": function,
                    "target": target,
                    "parent_manifest_sha256": parent_sha256,
                    "unresolved_record_sha256": unresolved_sha256,
                }
            )
            inventory.append(structural)
            unresolved = reverse.get("unresolved")
            evidence = (
                [item for item in unresolved if isinstance(item, str) and item]
                if isinstance(unresolved, list)
                else []
            )
            if evidence:
                item_authority = next(
                    (
                        require_object(raw_item, f"{event_id} item authority")
                        for raw_path in call_paths
                        for raw_item in require_list(
                            require_object(raw_path, f"{event_id} call path").get(
                                "items"
                            ),
                            f"{event_id} path items",
                        )
                        if require_object(raw_item, f"{event_id} item authority").get(
                            "item_key"
                        )
                        == target
                    ),
                    None,
                )
                if item_authority is None:
                    raise ReconciliationError(
                        f"{event_id} reverse target lacks serialized authority"
                    )
                expected_callers = reverse.get("expected_callers")
                found_calls = reverse.get("found_calls")
                exceptions.append(
                    {
                        "generated_digest_field": "generated_inventory_sha256",
                        "generated_digest": structural["canonical_leaf_sha256"],
                        "event_id": event_id,
                        "event_ordinal": event_ordinal,
                        "record_ordinal": record_ordinal,
                        "source": source,
                        "owner": owner,
                        "function": function,
                        "parent_manifest_sha256": parent_sha256,
                        "unresolved_record_sha256": unresolved_sha256,
                        "target": target,
                        "expected_callers": expected_callers,
                        "found_calls": found_calls,
                        "expected_callers_sha256": canonical_json_sha256(
                            expected_callers
                        ),
                        "found_calls_sha256": canonical_json_sha256(found_calls),
                        "unresolved_evidence": evidence,
                        "signature_sha256": item_authority.get("signature_sha256"),
                        "attribute_chain_sha256": item_authority.get(
                            "attribute_chain_sha256"
                        ),
                        "body_sha256": item_authority.get("body_sha256"),
                    }
                )
    return inventory, exceptions


def control_inventory(
    manifest: dict[str, object],
) -> tuple[list[dict[str, object]], list[dict[str, object]]]:
    inventory: list[dict[str, object]] = []
    exceptions: list[dict[str, object]] = []
    for event_ordinal, (event_id, raw_event) in enumerate(manifest.items(), start=1):
        event = require_object(raw_event, f"{event_id} control event")
        source = require_string(event.get("source"), f"{event_id} source")
        owner = require_string(event.get("owner"), f"{event_id} owner")
        function = require_string(event.get("function"), f"{event_id} function")
        parent_sha256 = canonical_json_sha256(event)
        record_ordinal = 0

        def add_atom(
            atom_kind: str,
            section_ordinal: int,
            atom_ordinal: int,
            ordered_section_sha256: str,
            unresolved_record: object,
            evidence: list[str],
        ) -> None:
            nonlocal record_ordinal
            record_ordinal += 1
            unresolved_sha256 = canonical_json_sha256(unresolved_record)
            structural = canonical_leaf(
                {
                    "layer": "CONTROL_ANCESTRY",
                    "atom_kind": atom_kind,
                    "event_id": event_id,
                    "event_ordinal": event_ordinal,
                    "record_ordinal": record_ordinal,
                    "source": source,
                    "owner": owner,
                    "function": function,
                    "section_ordinal": section_ordinal,
                    "atom_ordinal": atom_ordinal,
                    "ordered_section_sha256": ordered_section_sha256,
                    "parent_manifest_sha256": parent_sha256,
                    "unresolved_record_sha256": unresolved_sha256,
                }
            )
            inventory.append(structural)
            if evidence:
                exceptions.append(
                    {
                        "generated_digest_field": "generated_atom_sha256",
                        "generated_digest": structural["canonical_leaf_sha256"],
                        "event_id": event_id,
                        "event_ordinal": event_ordinal,
                        "record_ordinal": record_ordinal,
                        "source": source,
                        "owner": owner,
                        "function": function,
                        "parent_manifest_sha256": parent_sha256,
                        "unresolved_record_sha256": unresolved_sha256,
                        "section_ordinal": section_ordinal,
                        "atom_ordinal": atom_ordinal,
                        "atom_kind": atom_kind,
                        "ordered_section_sha256": ordered_section_sha256,
                        "unresolved_evidence": evidence,
                        "attribute_chain_sha256": event.get(
                            "owner_attribute_chain_sha256"
                        ),
                        "body_sha256": event.get("owner_body_sha256"),
                    }
                )

        owner_macros = require_list(
            event.get("owner_function_macros"), f"{event_id} owner macros"
        )
        owner_macros_sha256 = canonical_json_sha256(owner_macros)
        for atom_ordinal, raw_macro in enumerate(owner_macros, start=1):
            macro = require_object(raw_macro, f"{event_id} owner macro")
            evidence = (
                []
                if macro.get("event_owner_classification")
                == "ALLOWED_EVENT_OWNER_MACRO"
                else ["OWNER_MACRO_UNRESOLVED"]
            )
            add_atom(
                "OWNER_FUNCTION_MACRO",
                0,
                atom_ordinal,
                owner_macros_sha256,
                macro,
                evidence,
            )

        for collection_name, ordered_scope in (
            ("ordered_scopes", True),
            ("dominating_exits", False),
        ):
            sections = require_list(
                event.get(collection_name), f"{event_id} {collection_name}"
            )
            prefix = "SCOPE" if ordered_scope else "EXIT"
            valid_liveness = (
                {"RUNTIME_ROOTED_EDGE", "STATICALLY_REQUIRED_EDGE"}
                if ordered_scope
                else {"RUNTIME_ROOTED_EDGE"}
            )
            for section_ordinal, raw_section in enumerate(sections, start=1):
                section = require_object(raw_section, f"{event_id} control section")
                section_sha256 = canonical_json_sha256(section)
                macros = require_list(
                    section.get("macros", []), f"{event_id} section macros"
                )
                for atom_ordinal, macro in enumerate(macros, start=1):
                    add_atom(
                        f"{prefix}_MACRO",
                        section_ordinal,
                        atom_ordinal,
                        section_sha256,
                        macro,
                        ["SCOPE_MACRO_AUTHORITY_FORBIDDEN"],
                    )
                for field, suffix in (
                    ("resolved_values", "VALUE"),
                    ("resolved_authority", "AUTHORITY"),
                ):
                    atoms = require_list(section.get(field), f"{event_id} {field}")
                    for atom_ordinal, raw_atom in enumerate(atoms, start=1):
                        atom = require_object(raw_atom, f"{event_id} control atom")
                        evidence = (
                            [str(atom.get("authority", "UNRESOLVED"))]
                            if atom.get("classification") == "UNRESOLVED"
                            else []
                        )
                        add_atom(
                            f"{prefix}_{suffix}",
                            section_ordinal,
                            atom_ordinal,
                            section_sha256,
                            atom,
                            evidence,
                        )
                liveness = require_object(
                    section.get("edge_liveness"), f"{event_id} edge liveness"
                )
                classification = liveness.get("classification")
                add_atom(
                    f"{prefix}_LIVENESS",
                    section_ordinal,
                    1,
                    section_sha256,
                    liveness,
                    (
                        []
                        if classification in valid_liveness
                        else [str(classification or "UNRESOLVED_EDGE_LIVENESS")]
                    ),
                )

        collections = require_list(
            event.get("collection_mutations"), f"{event_id} collections"
        )
        for section_ordinal, raw_collection in enumerate(collections, start=1):
            collection = require_object(raw_collection, f"{event_id} collection")
            unresolved = require_list(
                collection.get("unresolved"), f"{event_id} collection unresolved"
            )
            add_atom(
                "COLLECTION_MUTATION",
                section_ordinal,
                1,
                canonical_json_sha256(collection),
                collection,
                [item for item in unresolved if isinstance(item, str) and item],
            )
    return inventory, exceptions


def reconcile_layer(
    manifest: object,
    ledger: object,
    layer: str,
) -> dict[str, object]:
    exact_manifest = require_object(manifest, f"{layer} manifest")
    exact_ledger = require_object(ledger, f"{layer} ledger")
    inventory, expected_exceptions = (
        entry_inventory(exact_manifest)
        if layer == "ENTRY_PATH"
        else control_inventory(exact_manifest)
    )
    records = require_list(exact_ledger.get("records"), f"{layer} records")
    if len(records) != len(expected_exceptions):
        raise ReconciliationError(f"{layer} exception record count differs")
    for ordinal, (raw_record, expected) in enumerate(
        zip(records, expected_exceptions), start=1
    ):
        record = require_object(raw_record, f"{layer} exception {ordinal}")
        generated_field = require_string(
            expected["generated_digest_field"], f"{layer} generated digest field"
        )
        if record.get(generated_field) != expected["generated_digest"]:
            raise ReconciliationError(
                f"{layer} exception {ordinal} generated leaf differs"
            )
        for field, value in expected.items():
            if field in {"generated_digest_field", "generated_digest"}:
                continue
            if record.get(field) != value:
                raise ReconciliationError(
                    f"{layer} exception {ordinal} {field} differs"
                )
        canonical = record.get("canonical_leaf_sha256")
        payload = dict(tuple(record.items())[:-1])
        if canonical != canonical_json_sha256(payload):
            raise ReconciliationError(
                f"{layer} exception {ordinal} canonical digest differs"
            )
    complete_digests = [record["canonical_leaf_sha256"] for record in inventory]
    generated_digests = [
        expected["generated_digest"] for expected in expected_exceptions
    ]
    if len(set(generated_digests)) != len(generated_digests):
        raise ReconciliationError(f"{layer} exception leaves are duplicated")
    if not set(generated_digests).issubset(set(complete_digests)):
        raise ReconciliationError(
            f"{layer} exception is outside the complete inventory"
        )
    exception_set = set(generated_digests)
    static_digests = [
        digest for digest in complete_digests if digest not in exception_set
    ]
    expected_top = {
        "complete_manifest_sha256": canonical_json_sha256(exact_manifest),
        "complete_inventory_sha256": canonical_json_sha256(complete_digests),
        "static_pass_sha256": canonical_json_sha256(static_digests),
        "exceptions_sha256": canonical_json_sha256(
            [record["canonical_leaf_sha256"] for record in records]
        ),
        "complete_count": len(complete_digests),
        "static_pass_count": len(static_digests),
        "exception_count": len(records),
    }
    for field, value in expected_top.items():
        if exact_ledger.get(field) != value:
            raise ReconciliationError(f"{layer} ledger {field} differs")
    event_counts = require_object(
        exact_ledger.get("event_counts"), f"{layer} event counts"
    )
    if tuple(event_counts) != tuple(exact_manifest):
        raise ReconciliationError(f"{layer} event-count order differs")
    for event_id, raw_counts in event_counts.items():
        counts = require_object(raw_counts, f"{layer} {event_id} counts")
        event_inventory = [
            record for record in inventory if record["event_id"] == event_id
        ]
        event_exceptions = [
            record for record in records if record.get("event_id") == event_id
        ]
        expected_event = {
            "complete": len(event_inventory),
            "static_pass": len(event_inventory) - len(event_exceptions),
            "exceptions": len(event_exceptions),
            "ordered_exception_leaf_sha256": canonical_json_sha256(
                [record["canonical_leaf_sha256"] for record in event_exceptions]
            ),
        }
        if counts != expected_event:
            raise ReconciliationError(f"{layer} {event_id} event partition differs")
    return expected_top


def reconcile_test_plan(plan: dict[str, object]) -> dict[str, object]:
    entry = reconcile_layer(
        plan.get("limit_event_entry_call_paths"),
        plan.get("limit_event_entry_exception_ledger"),
        "ENTRY_PATH",
    )
    control = reconcile_layer(
        plan.get("limit_event_control_ancestries"),
        plan.get("limit_event_control_exception_ledger"),
        "CONTROL_ANCESTRY",
    )
    return {
        "entry_complete_count": entry["complete_count"],
        "entry_static_pass_count": entry["static_pass_count"],
        "entry_exception_count": entry["exception_count"],
        "control_complete_count": control["complete_count"],
        "control_static_pass_count": control["static_pass_count"],
        "control_exception_count": control["exception_count"],
    }


def synthetic_entry_plan() -> dict[str, object]:
    digest = "a" * 64
    event_id = "synthetic.event"
    callsite = {
        "caller": "crates/example/src/lib.rs:Example:recover",
        "callee": "crates/example/src/lib.rs:Example:recover_inner",
        "unresolved": ["synthetic unresolved call"],
    }
    manifest = {
        event_id: {
            "event_owner": "crates/example/src/lib.rs:Example:recover_inner",
            "call_paths": [
                {
                    "entry_root": "crates/example/src/lib.rs:Example:recover",
                    "items": [
                        {
                            "item_key": "crates/example/src/lib.rs:Example:recover",
                            "signature_sha256": digest,
                            "attribute_chain_sha256": digest,
                            "body_sha256": digest,
                        },
                        {
                            "item_key": (
                                "crates/example/src/lib.rs:Example:recover_inner"
                            ),
                            "signature_sha256": digest,
                            "attribute_chain_sha256": digest,
                            "body_sha256": digest,
                        },
                    ],
                    "callsites": [callsite],
                }
            ],
            "reverse_callers": {},
            "unresolved": ["synthetic unresolved call"],
        }
    }
    inventory, expected = entry_inventory(manifest)
    generated = expected[0]
    exception = {
        key: value
        for key, value in generated.items()
        if key not in {"generated_digest_field", "generated_digest"}
    }
    exception["generated_edge_sha256"] = generated["generated_digest"]
    exception["canonical_leaf_sha256"] = canonical_json_sha256(exception)
    complete_digests = [inventory[0]["canonical_leaf_sha256"]]
    event_exception_digests = [exception["canonical_leaf_sha256"]]
    ledger = {
        "complete_manifest_sha256": canonical_json_sha256(manifest),
        "complete_inventory_sha256": canonical_json_sha256(complete_digests),
        "static_pass_sha256": canonical_json_sha256([]),
        "exceptions_sha256": canonical_json_sha256(event_exception_digests),
        "complete_count": 1,
        "static_pass_count": 0,
        "exception_count": 1,
        "event_counts": {
            event_id: {
                "complete": 1,
                "static_pass": 0,
                "exceptions": 1,
                "ordered_exception_leaf_sha256": canonical_json_sha256(
                    event_exception_digests
                ),
            }
        },
        "records": [exception],
    }
    return {
        "limit_event_entry_call_paths": manifest,
        "limit_event_entry_exception_ledger": ledger,
    }


def require_self_tests() -> None:
    exact = synthetic_entry_plan()
    reconcile_layer(
        exact["limit_event_entry_call_paths"],
        exact["limit_event_entry_exception_ledger"],
        "ENTRY_PATH",
    )
    for field, replacement in (
        ("complete_count", 2),
        ("complete_inventory_sha256", "0" * 64),
        ("static_pass_sha256", "0" * 64),
    ):
        hostile = json.loads(json.dumps(exact))
        hostile["limit_event_entry_exception_ledger"][field] = replacement
        try:
            reconcile_layer(
                hostile["limit_event_entry_call_paths"],
                hostile["limit_event_entry_exception_ledger"],
                "ENTRY_PATH",
            )
        except ReconciliationError:
            continue
        raise ReconciliationError(f"self-test accepted hostile {field}")
    hostile = json.loads(json.dumps(exact))
    hostile["limit_event_entry_exception_ledger"]["records"][0][
        "generated_edge_sha256"
    ] = "0" * 64
    try:
        reconcile_layer(
            hostile["limit_event_entry_call_paths"],
            hostile["limit_event_entry_exception_ledger"],
            "ENTRY_PATH",
        )
    except ReconciliationError:
        return
    raise ReconciliationError("self-test accepted a hostile generated edge")


def main(arguments: list[str]) -> None:
    if arguments == ["--self-test"]:
        require_self_tests()
        print("S20-530 exception reconciliation self-test: PASS")
        return
    if len(arguments) > 1:
        raise ReconciliationError(
            "usage: reconcile_s20_530_exception_ledgers.py [test-plan.json]"
        )
    path = Path(arguments[0]).resolve() if arguments else DEFAULT_TEST_PLAN
    result = reconcile_test_plan(load_strict_json(path))
    print(
        "S20-530 exception reconciliation: PASS "
        + json.dumps(result, sort_keys=True, separators=(",", ":"))
    )


if __name__ == "__main__":
    try:
        main(sys.argv[1:])
    except (OSError, ReconciliationError) as error:
        print(f"S20-530 exception reconciliation failed: {error}", file=sys.stderr)
        raise SystemExit(1)
