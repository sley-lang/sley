#!/usr/bin/env python3
"""Build the exact S20-530 matrix test plan from frozen checker authority."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
from dataclasses import dataclass
from pathlib import Path

import check_s20_530_crash_recovery as contract


@dataclass(frozen=True)
class TestDefinition:
    source: str
    name: str
    body: str
    qualified: str


def normalized_name(value: str) -> str:
    return re.sub(r"[^a-z0-9]+", "", value.lower())


def source_test_definitions(relative: str, source: str) -> tuple[TestDefinition, ...]:
    mask = contract.rust_code_mask(source)
    module_range = contract.exact_test_module_range(source, mask)
    if module_range is None:
        raise RuntimeError(f"{relative} lacks one exact test module")
    module_opening, module_closing = module_range
    pattern = re.compile(
        rf"(?P<attrs>(?:#\[[^\]]+\]\s*)+)fn\s+"
        rf"(?P<name>{contract.RUST_IDENTIFIER})\s*\("
    )
    prefix = {
        "crates/sley-store/src/lib.rs": "",
        "crates/sley-txn/src/repository.rs": "repository",
        "crates/sley-repo/src/refs.rs": "refs",
        "crates/sley-repo/src/gc.rs": "gc",
    }[relative]
    definitions: list[TestDefinition] = []
    for match in pattern.finditer(source):
        fn_start = source.find("fn", match.end("attrs"), match.end())
        if (
            fn_start < 0
            or not (module_opening < match.start("attrs") < module_closing)
            or not all(mask[match.start("attrs") : match.end()])
            or contract.has_preceding_code_attribute(source, mask, match.start("attrs"))
            or contract.brace_depth_at(source, mask, fn_start) != 1
            or re.sub(r"\s+", "", match["attrs"]) != "#[test]"
        ):
            continue
        parameter_opening = source.rfind("(", fn_start, match.end())
        opening = contract.rust_function_body_opening(
            source, mask, parameter_opening, module_closing
        )
        if opening is None:
            continue
        closing = contract.matching_delimiter(source, mask, opening, "{", "}")
        if closing is None or closing >= module_closing:
            continue
        name = contract.semantic_rust_identifier(match["name"])
        local = f"tests::{name}"
        qualified = f"{prefix}::{local}" if prefix else local
        definitions.append(
            TestDefinition(relative, name, source[opening + 1 : closing], qualified)
        )
    return tuple(definitions)


class TestInventory:
    def __init__(self) -> None:
        self.sources = {
            relative: (contract.ROOT / relative).read_text(encoding="utf-8")
            for relative in contract.OWNER_SOURCES
        }
        definitions = tuple(
            definition
            for relative, source in self.sources.items()
            for definition in source_test_definitions(relative, source)
        )
        by_name: dict[str, TestDefinition] = {}
        for definition in definitions:
            if definition.name in by_name:
                raise RuntimeError(f"duplicate local test name: {definition.name}")
            by_name[definition.name] = definition
        self.definitions = definitions
        self.by_name = by_name

    def resolve(
        self,
        row_id: str,
        subcase_id: str | None = None,
        leaf_id: str | None = None,
    ) -> TestDefinition:
        explicit = {
            (row, subcase, None): name
            for row, subcase, name, _sha256 in contract.GC_RECOVERY_TEST_AUTHORITIES
        }
        explicit.update(
            {
                ("CROSS-01", None, None): (
                    "cross01_old_accepted_old_branch_recoveries_are_idempotent"
                ),
                ("CROSS-02", None, None): (
                    "cross02_old_accepted_new_branch_recoveries_are_idempotent"
                ),
                ("CROSS-03", None, None): (
                    "cross03_new_accepted_old_branch_recoveries_are_idempotent"
                ),
                ("CROSS-04", None, None): (
                    "cross04_new_accepted_new_branch_recoveries_are_idempotent"
                ),
                ("CROSS-05", "caller_held_exclusive_sequence", None): (
                    "exclusive_recovery_ownership_blocks_shared_operations"
                ),
                ("CROSS-05", "transaction_no_arg_wrapper", None): (
                    "cross05_transaction_no_arg_wrapper_holds_exclusive_maintenance"
                ),
                ("CROSS-05", "ref_no_arg_wrapper", None): (
                    "ref_no_argument_recovery_holds_exclusive_maintenance"
                ),
            }
        )
        key = (row_id, subcase_id, leaf_id)
        if key in explicit:
            candidates = [self.by_name[explicit[key]]]
        else:
            row = normalized_name(row_id)
            subcase = normalized_name(subcase_id or "")
            leaf = normalized_name(leaf_id or "")
            candidates = [
                definition
                for definition in self.definitions
                if normalized_name(definition.name).startswith(row)
                and (not subcase or subcase in normalized_name(definition.name))
                and (not leaf or leaf in normalized_name(definition.name))
            ]
        allowed = set(contract.allowed_test_sources(row_id))
        candidates = [item for item in candidates if item.source in allowed]
        if len(candidates) != 1:
            names = [item.name for item in candidates]
            raise RuntimeError(
                f"{row_id}/{subcase_id}/{leaf_id} maps to {len(candidates)} tests: "
                f"{names}"
            )
        return candidates[0]


def direct_assertions(body: str) -> list[str]:
    assertions = [
        statement
        for _start, _end, statement in contract.top_level_statement_ranges(body)
        if contract.ASSERTION_MACRO.match(statement) is not None
    ]
    return list(dict.fromkeys(assertions))


def only(label: str, values: list[str]) -> str:
    if len(values) != 1:
        raise RuntimeError(f"{label} has {len(values)} candidates")
    return values[0]


def normalized_assertion(assertions: list[str], expected: str, label: str) -> str:
    normalized = contract.normalize_rust_tokens(expected)
    return only(
        label,
        [
            assertion
            for assertion in assertions
            if contract.normalize_rust_tokens(assertion) == normalized
        ],
    )


def comparison_assertion(
    assertions: list[str], left: str, right: str, label: str
) -> str:
    expected = {
        contract.code_only_normalized(left),
        contract.code_only_normalized(right),
    }
    candidates: list[str] = []
    for assertion in assertions:
        parsed = contract.exact_comparison_assertion(assertion)
        if parsed is None:
            continue
        _macro, operands = parsed
        if {contract.code_only_normalized(value) for value in operands} == expected:
            candidates.append(assertion)
    return only(label, candidates)


def proof_binding_sha256(*parts: str) -> str:
    row_id, subcase_id, leaf_id, subject, assertion = parts
    return contract.preflight_equality_proof_sha256(
        row_id,
        subcase_id or None,
        leaf_id or None,
        subject,
        assertion,
    )


def preflight_canary_fields(
    row_id: str,
    subcase_id: str | None,
    leaf_id: str | None,
    assertions: list[str],
) -> dict[str, object]:
    names = contract.PREFLIGHT_CANARIES[row_id]
    hashes: dict[str, str] = {}
    kinds: dict[str, str] = {}
    mapping: dict[str, str] = {}
    for name in names:
        bytes_assertion = comparison_assertion(
            assertions,
            f"{name}_before_snapshot",
            f"{name}_after_snapshot",
            f"{row_id}/{subcase_id}/{leaf_id} {name} bytes",
        )
        kind_assertion = comparison_assertion(
            assertions,
            f"{name}_before_kind",
            f"{name}_after_kind",
            f"{row_id}/{subcase_id}/{leaf_id} {name} kind",
        )
        expected_kind = contract.PREFLIGHT_CANARY_KINDS.get(
            (row_id, subcase_id, name), "regular"
        )
        expected_kind_assertion = normalized_assertion(
            assertions,
            f"::core::assert_eq!({name}_before_kind, {json.dumps(expected_kind)});",
            f"{row_id}/{subcase_id}/{leaf_id} {name} expected kind",
        )
        digest = proof_binding_sha256(
            row_id, subcase_id or "", leaf_id or "", name, bytes_assertion
        )
        hashes[f"{name}_before_sha256"] = digest
        hashes[f"{name}_after_sha256"] = digest
        kinds[f"{name}_before_kind"] = expected_kind
        kinds[f"{name}_after_kind"] = expected_kind
        mapping[f"{name}_bytes"] = bytes_assertion
        mapping[f"{name}_kind"] = kind_assertion
        mapping[f"{name}_expected_kind"] = expected_kind_assertion
        frozen_fixture = contract.PREFLIGHT_CANARY_FIXTURE_ASSERTIONS.get(
            (row_id, subcase_id, name)
        )
        if frozen_fixture is not None:
            mapping[f"{name}_fixture"] = normalized_assertion(
                assertions,
                frozen_fixture,
                f"{row_id}/{subcase_id}/{leaf_id} {name} fixture",
            )
    tree_assertion = comparison_assertion(
        assertions,
        "owner_tree_before_snapshot",
        "owner_tree_after_snapshot",
        f"{row_id}/{subcase_id}/{leaf_id} owner tree",
    )
    tree_digest = proof_binding_sha256(
        row_id, subcase_id or "", leaf_id or "", "owner_tree", tree_assertion
    )
    hashes["owner_tree_before_sha256"] = tree_digest
    hashes["owner_tree_after_sha256"] = tree_digest
    mapping["owner_tree"] = tree_assertion
    return {
        "cleanup_canary_hashes": hashes,
        "cleanup_canary_kinds": kinds,
        "cleanup_canary_assertions": mapping,
    }


def owned_multifault_preflight_fields(
    row_id: str,
    subcase_id: str,
) -> dict[str, object]:
    key = (row_id, subcase_id, None)
    canary = contract.PREFLIGHT_CANARIES[row_id][0]
    canary_assertions = contract.multifault_owned_entry_canary_assertions(key)
    tree_assertion = contract.multifault_snapshot_assertions()[
        "operation_1_tree_unchanged"
    ]
    canary_digest = proof_binding_sha256(
        row_id,
        subcase_id,
        "",
        canary,
        canary_assertions["snapshot_unchanged"],
    )
    tree_digest = proof_binding_sha256(
        row_id,
        subcase_id,
        "",
        "owner_tree",
        tree_assertion,
    )
    return {
        "cleanup_canary_hashes": {
            f"{canary}_before_sha256": canary_digest,
            f"{canary}_after_sha256": canary_digest,
            "owner_tree_before_sha256": tree_digest,
            "owner_tree_after_sha256": tree_digest,
        },
        "cleanup_canary_kinds": {
            f"{canary}_before_kind": "regular",
            f"{canary}_after_kind": "regular",
        },
        "cleanup_canary_assertions": {
            f"{canary}_bytes": canary_assertions["snapshot_unchanged"],
            f"{canary}_kind": canary_assertions["kind_unchanged"],
            f"{canary}_expected_kind": canary_assertions["expected_kind"],
            "owner_tree": tree_assertion,
        },
    }


def multifault_fields(
    row_id: str, subcase_id: str, leaf_id: str | None
) -> dict[str, object]:
    key = (row_id, subcase_id, leaf_id)
    spec = contract.MULTIFAULT_OVERLAYS[key]
    primary_fixture = contract.multifault_fixture_assertions("primary", spec.primary)
    secondary_fixture = contract.multifault_fixture_assertions(
        "secondary", spec.secondary
    )
    primary_probe = {
        fact: contract.multifault_probe_assertion("primary", fact, spec.winner)
        for fact in contract.multifault_overlay_probe_fields(spec.primary)
    }
    secondary_probe = {
        fact: contract.multifault_probe_assertion("secondary", fact, spec.loser)
        for fact in contract.multifault_overlay_probe_fields(spec.secondary)
    }
    return {
        "multifault_plan_sha256": contract.multifault_plan_sha256(key, {}),
        "primary_fixture_assertions": primary_fixture,
        "secondary_fixture_assertions": secondary_fixture,
        "distinctness_assertions": {
            spec.distinctness_class: contract.multifault_distinctness_assertion(spec)
        },
        "primary_probe_assertions": primary_probe,
        "secondary_probe_assertions": secondary_probe,
        "m2_operation_bindings": contract.multifault_operation_bindings(key, spec),
        "m2_result_assertions": contract.multifault_result_assertions(
            spec, spec.owner_source
        ),
        "m2_repair_assertions": contract.multifault_repair_assertions(spec),
        "m2_cycle_epoch_assertions": contract.multifault_cycle_epoch_assertions(spec),
        "m2_snapshot_assertions": contract.multifault_snapshot_assertions(),
        "m2_owned_entry_canary_assertions": (
            contract.multifault_owned_entry_canary_assertions(key)
        ),
    }


def recorded_let(body: str, identifier: str, label: str) -> str:
    bindings = contract.top_level_let_statement_ranges(body, identifier)
    if len(bindings) != 1:
        raise RuntimeError(f"{label} has {len(bindings)} bindings for {identifier}")
    return bindings[0][2]


def durability_entry(row_id: str, definition: TestDefinition) -> dict[str, object]:
    assertions = direct_assertions(definition.body)
    operation_keys = contract.durability_operation_keys(row_id)
    operation_bindings = {
        operation: recorded_let(
            definition.body, f"{operation}_result", f"{row_id} {operation}"
        )
        for operation in operation_keys
    }
    fault_keys = tuple(
        operation
        for operation in operation_keys
        if operation in {"first_fault", "second_fault"}
    )
    result_bindings = {
        f"{fault}_error": recorded_let(
            definition.body, f"{fault}_error", f"{row_id} {fault} error"
        )
        for fault in fault_keys
    }
    expected_code = contract.DURABILITY_RESULT_CODES[row_id]
    result_assertions: dict[str, str] = {}
    for fault in fault_keys:
        error = f"{fault}_error"
        expected = (
            f"::core::assert_eq!("
            f"{contract.durability_result_projection(row_id, error)}, "
            f"{json.dumps(expected_code)});"
        )
        result_assertions[f"{fault}_code"] = normalized_assertion(
            assertions, expected, f"{row_id} {fault} result"
        )
    non_fault_keys = tuple(
        operation for operation in operation_keys if operation not in fault_keys
    )
    for operation in non_fault_keys:
        expected = f"::core::assert!({operation}_result.is_ok());"
        result_assertions[f"{operation}_success"] = normalized_assertion(
            assertions, expected, f"{row_id} {operation} success"
        )

    phases = contract.durability_snapshot_phases(row_id)
    tree_assertions = [
        comparison_assertion(
            assertions,
            f"{left}_owner_tree_snapshot",
            f"{right}_owner_tree_snapshot",
            f"{row_id} tree {left}/{right}",
        )
        for left, right in zip(phases, phases[1:])
    ]
    path_assertions = [
        comparison_assertion(
            assertions,
            f"{left}_{path}_path_snapshot",
            f"{right}_{path}_path_snapshot",
            f"{row_id} {path} {left}/{right}",
        )
        for path in ("primary", "secondary")
        for left, right in zip(phases, phases[1:])
    ]
    report_assertions: list[str] = []
    covered: set[str] = set()
    for assertion in assertions:
        if contract.exact_assert_eq_operands(assertion) is None:
            continue
        local = {
            operation
            for operation in non_fault_keys
            if contract.code_contains_token(assertion, f"{operation}_result")
        }
        if len(local) == 1:
            report_assertions.append(assertion)
            covered.update(local)
    if covered != set(non_fault_keys):
        raise RuntimeError(f"{row_id} report coverage differs: {sorted(covered)}")

    entry = {
        "result": "PASS",
        "fresh_fixture": True,
        "protocol": contract.durability_retry_protocol(row_id),
        "expected_result": expected_code,
        "tests": [definition.name],
        "assertions": assertions,
        "operation_bindings": operation_bindings,
        "result_bindings": result_bindings,
        "result_assertions": result_assertions,
        "tree_assertions": tree_assertions,
        "path_assertions": path_assertions,
        "report_assertions": report_assertions,
    }
    problem = contract.durability_evidence_problem(
        row_id, entry, definition.source, definition.body
    )
    if problem is not None:
        raise RuntimeError(f"{row_id} durability evidence: {problem}")
    return entry


def recovery_success_entry(
    row_id: str, definition: TestDefinition
) -> dict[str, object]:
    reports = contract.recovery_success_report_assertions(row_id)
    pointers = contract.recovery_success_pointer_assertions(row_id)
    closure = contract.recovery_success_closure_assertions(row_id)
    entry = {
        "result": "PASS",
        "fresh_fixture": True,
        "tests": [definition.name],
        "assertions": direct_assertions(definition.body),
        "operation_bindings": contract.recovery_success_operation_bindings(row_id),
        "report_assertions": reports,
        "pointer_assertions": pointers,
        "closure_assertions": closure,
    }
    problem = contract.recovery_success_evidence_problem(
        row_id, entry, definition.source, definition.body
    )
    if problem is not None:
        raise RuntimeError(f"{row_id} success evidence: {problem}")
    return entry


def gc_witness_entry(row_id: str, definition: TestDefinition) -> dict[str, object]:
    assertions = direct_assertions(definition.body)
    semantic = {
        field: normalized_assertion(
            assertions,
            f"::core::assert!({field});",
            f"{row_id} {field}",
        )
        for field in ("immediate_state_verified", "recovery_result_verified")
    }
    return {
        "result": "PASS",
        "fresh_fixture": True,
        "tests": [definition.name],
        "assertions": assertions,
        "immediate_state_verified": True,
        "recovery_result_verified": True,
        "semantic_assertions": semantic,
    }


def cross05_entry(
    row_id: str, subcase_id: str, definition: TestDefinition
) -> dict[str, object]:
    del row_id
    assertions = direct_assertions(definition.body)
    semantic = {
        field: normalized_assertion(
            assertions,
            f"::core::assert!({field});",
            f"CROSS-05/{subcase_id} {field}",
        )
        for field in contract.CROSS_05_EVIDENCE_FIELDS
    }
    entry = {
        "result": "PASS",
        "fresh_fixture": True,
        **{field: True for field in contract.CROSS_05_EVIDENCE_FIELDS},
        "semantic_assertions": semantic,
        "tests": [definition.name],
        "assertions": assertions,
    }
    problem = contract.cross05_evidence_problem(entry)
    if problem is not None:
        raise RuntimeError(f"CROSS-05/{subcase_id}: {problem}")
    problem = contract.cross05_test_body_problem(definition.body, subcase_id, semantic)
    if problem is not None:
        raise RuntimeError(f"CROSS-05/{subcase_id} body: {problem}")
    return entry


def gc_partial_entry(
    row_id: str, subcase_id: str, definition: TestDefinition
) -> dict[str, object]:
    assertions = direct_assertions(definition.body)
    expected: dict[str, object] = {
        "expected_result": "GC_DELETE_IO",
        "candidate_count": 3,
        "candidate_order_verified": True,
        "failed_candidate_index": 1,
        "durable_prefix_count": 1,
        "original_deletion_candidates_exact": True,
        "reachable_objects_unchanged": True,
        "deleted_objects_exact_prefix": True,
        "failed_object_exact_current": True,
        "failed_current_excluded": True,
        "suffix_untouched": True,
        "retry_complete": True,
        "third_run_empty": True,
        "failed_current_present": row_id == "GC-01",
    }
    if row_id == "GC-01":
        expected["retry_deleted_exact_remaining"] = True
    else:
        expected["retry_resynced_failed_leaf_before_replan"] = True
        expected["retry_deleted_exact_suffix"] = True
        expected["retry_did_not_claim_failed_current"] = True
    semantic = {
        field: only(
            f"{row_id}/{subcase_id} {field}",
            [
                assertion
                for assertion in assertions
                if contract.exact_value_assertion_problem(assertion, field, value)
                is None
            ],
        )
        for field, value in expected.items()
    }
    return {
        "result": "PASS",
        "fresh_fixture": True,
        "tests": [definition.name],
        "assertions": assertions,
        **expected,
        "semantic_assertions": semantic,
    }


def error_entry(
    row_id: str,
    subcase_id: str | None,
    leaf_id: str | None,
    definition: TestDefinition,
) -> dict[str, object]:
    assertions = direct_assertions(definition.body)
    if leaf_id is None:
        expected_code, expected_variant, expected_chain = contract.error_case_spec(
            row_id, subcase_id
        )
    else:
        expected_code, expected_variant, expected_chain = (
            contract.grouped_error_case_spec(row_id, str(subcase_id), leaf_id)
        )
    multifault_key = (row_id, str(subcase_id), leaf_id)
    if multifault_key in contract.MULTIFAULT_OVERLAYS:
        semantic = contract.multifault_base_error_semantic_assertions(
            multifault_key, definition.source
        )
    else:
        semantic = {
            "expected_result": only(
                f"{row_id}/{subcase_id}/{leaf_id} result",
                [
                    assertion
                    for assertion in assertions
                    if contract.exact_error_result_assertion_problem(
                        assertion,
                        definition.source,
                        expected_variant,
                        expected_code,
                    )
                    is None
                ],
            ),
            "expected_variant": only(
                f"{row_id}/{subcase_id}/{leaf_id} variant",
                [
                    assertion
                    for assertion in assertions
                    if contract.exact_error_variant_assertion_problem(
                        assertion, definition.source, expected_variant
                    )
                    is None
                ],
            ),
            "expected_source_chain": only(
                f"{row_id}/{subcase_id}/{leaf_id} source chain",
                [
                    assertion
                    for assertion in assertions
                    if contract.exact_error_source_assertion_problem(
                        assertion, definition.source, expected_chain
                    )
                    is None
                ],
            ),
            "no_mutation": comparison_assertion(
                assertions,
                "owner_tree_before_snapshot",
                "owner_tree_after_snapshot",
                f"{row_id}/{subcase_id}/{leaf_id} no mutation",
            ),
            "no_success_report": only(
                f"{row_id}/{subcase_id}/{leaf_id} no success",
                [
                    assertion
                    for assertion in assertions
                    if contract.exact_assert_predicate(assertion) == "result.is_err()"
                ],
            ),
        }
    values: dict[str, object] = {
        "result": "PASS",
        "fresh_fixture": True,
        "tests": [definition.name],
        "assertions": assertions,
        "expected_result": expected_code,
        "expected_variant": expected_variant,
        "expected_source_chain": list(expected_chain),
        "no_mutation": True,
        "no_success_report": True,
        "semantic_assertions": semantic,
    }
    if leaf_id is not None:
        fixture_digest, rendered = contract.corruption_fixture_plan(
            (row_id, str(subcase_id), leaf_id)
        )
        values["fixture_plan_sha256"] = fixture_digest
        values["fixture_assertions"] = rendered.fact_assertions
    provenance_key = (row_id, subcase_id)
    if provenance_key in contract.RECOVERY_PROVENANCE_SPECS:
        spec = contract.RECOVERY_PROVENANCE_SPECS[provenance_key]
        values["fixture_plan_sha256"] = contract.recovery_provenance_plan_sha256(
            provenance_key, spec
        )
        values["fixture_assertions"] = contract.recovery_provenance_fact_assertions(
            provenance_key, spec
        )
    if multifault_key in contract.MULTIFAULT_OVERLAYS:
        values.update(multifault_fields(row_id, str(subcase_id), leaf_id))
    if (
        row_id in contract.PREFLIGHT_CANARIES
        and multifault_key not in contract.MULTIFAULT_OVERLAYS
    ):
        values.update(preflight_canary_fields(row_id, subcase_id, leaf_id, assertions))
    fields = contract.error_evidence_field_order(row_id, subcase_id, leaf_id)
    missing = tuple(field for field in fields if field not in values)
    if missing:
        raise RuntimeError(f"{row_id}/{subcase_id}/{leaf_id} missing fields: {missing}")
    entry = {field: values[field] for field in fields}
    if multifault_key not in contract.MULTIFAULT_OVERLAYS:
        problem = contract.operation_error_binding_problem(
            definition.body,
            definition.source,
            row_id,
            subcase_id,
            semantic,
            leaf_id,
        )
        if problem is not None:
            raise RuntimeError(
                f"{row_id}/{subcase_id}/{leaf_id} operation binding: {problem}"
            )
    if leaf_id is not None and multifault_key not in contract.MULTIFAULT_OVERLAYS:
        problem = contract.corruption_fixture_evidence_problem(
            (row_id, str(subcase_id), leaf_id),
            entry,
            definition.source,
            definition.body,
        )
        if problem is not None:
            raise RuntimeError(f"{row_id}/{subcase_id}/{leaf_id} fixture: {problem}")
    if provenance_key in contract.RECOVERY_PROVENANCE_SPECS:
        problem = contract.recovery_provenance_evidence_problem(
            provenance_key, entry, definition.source, definition.body
        )
        if problem is not None:
            raise RuntimeError(f"{row_id}/{subcase_id} provenance: {problem}")
    if multifault_key in contract.MULTIFAULT_OVERLAYS:
        problem = contract.multifault_evidence_problem(
            row_id, str(subcase_id), entry, definition.source, leaf_id
        )
        if problem is not None:
            raise RuntimeError(f"{row_id}/{subcase_id}/{leaf_id} multifault: {problem}")
        problem = contract.multifault_operation_binding_problem(
            definition.body, row_id, str(subcase_id), entry, leaf_id
        )
        if problem is not None:
            raise RuntimeError(
                f"{row_id}/{subcase_id}/{leaf_id} multifault body: {problem}"
            )
    if (
        row_id in contract.PREFLIGHT_CANARIES
        and multifault_key not in contract.MULTIFAULT_OVERLAYS
    ):
        problem = contract.preflight_canary_problem(row_id, subcase_id, entry, leaf_id)
        if problem is not None:
            raise RuntimeError(f"{row_id}/{subcase_id}/{leaf_id} canary: {problem}")
    return entry


def owned_entry(
    row_id: str, subcase_id: str, definition: TestDefinition
) -> dict[str, object]:
    assertions = direct_assertions(definition.body)
    fatal = subcase_id in contract.OWNED_ENTRY_FATAL_SUBCASES
    if subcase_id == "owned_stage":
        expected_result = "PASS_REMOVED_AND_SYNCED"
        owned_stage_removed = True
        preserved = False
        no_mutation = False
    else:
        expected_result = (
            contract.OWNED_ENTRY_ERROR_CODES[row_id] if fatal else "PASS_PRESERVED"
        )
        owned_stage_removed = False
        preserved = True
        no_mutation = True
    entry: dict[str, object] = {
        "result": "PASS",
        "fresh_fixture": True,
        "tests": [definition.name],
        "assertions": assertions,
        "expected_result": expected_result,
        "owned_stage_removed": owned_stage_removed,
        "preserved": preserved,
        "no_mutation": no_mutation,
        "semantic_assertions": contract.owned_entry_semantic_assertion_map(
            row_id, subcase_id, definition.source
        ),
    }
    if (row_id, subcase_id) in contract.OWNED_ENTRY_MULTIFAULT_CASES:
        entry.update(multifault_fields(row_id, subcase_id, None))
    else:
        specs = {
            str(spec["name"]): spec
            for spec in contract.owned_entry_fixture_specs(row_id, subcase_id)
            if spec["coverage"] is True
        }
        entry["coverage_assertions"] = {
            name: contract.owned_entry_coverage_assertion(spec)
            for name, spec in specs.items()
        }
    if fatal:
        if (row_id, subcase_id) in contract.OWNED_ENTRY_MULTIFAULT_CASES:
            entry.update(owned_multifault_preflight_fields(row_id, subcase_id))
        else:
            entry.update(preflight_canary_fields(row_id, subcase_id, None, assertions))
    problem = contract.owned_entry_semantic_evidence_problem(
        row_id, subcase_id, entry, definition.source, definition.body
    )
    if problem is not None:
        raise RuntimeError(f"{row_id}/{subcase_id} owned semantics: {problem}")
    if (row_id, subcase_id) in contract.OWNED_ENTRY_MULTIFAULT_CASES:
        problem = contract.owned_entry_multifault_preflight_problem(
            row_id, subcase_id, entry, definition.body
        )
        if problem is not None:
            raise RuntimeError(f"{row_id}/{subcase_id} owned canary: {problem}")
        problem = contract.multifault_evidence_problem(
            row_id, subcase_id, entry, definition.source
        )
        if problem is not None:
            raise RuntimeError(f"{row_id}/{subcase_id} multifault: {problem}")
    else:
        problem = contract.owned_entry_operation_binding_problem(
            definition.body, definition.source, row_id, subcase_id
        )
        if problem is not None:
            raise RuntimeError(f"{row_id}/{subcase_id} owned body: {problem}")
    return entry


def limit_entry(
    row_id: str, subcase_id: str, definition: TestDefinition
) -> dict[str, object]:
    assertions = direct_assertions(definition.body)
    _owner, _constant, frozen_default = contract.LIMIT_DEFAULTS[row_id][subcase_id]
    constant = contract.limit_default_assertion_constant(row_id, subcase_id)
    expected_limit = contract.limit_plus_one_expected_result(row_id, subcase_id)
    frozen_default_assertion = only(
        f"{row_id}/{subcase_id} frozen default",
        [
            assertion
            for assertion in assertions
            if (
                (operands := contract.exact_assert_eq_operands(assertion)) is not None
                and {
                    contract.code_only_normalized(operands[0]),
                    contract.code_only_normalized(operands[1]),
                }
                == {constant, f"{frozen_default:_}"}
            )
        ],
    )
    if contract.is_transaction_owned_branch_limit(row_id, subcase_id):
        expected_predicate = contract.code_only_normalized(
            "::core::matches!(&limit_plus_one_error, "
            "super::RecoveryAncestryError::LimitExceeded)"
        )
        limit_assertion = only(
            f"{row_id}/{subcase_id} limit variant",
            [
                assertion
                for assertion in assertions
                if contract.exact_assert_predicate(assertion) == expected_predicate
            ],
        )
    else:
        projection = {
            "LIMIT-01": "limit_plus_one_error.symbol()",
            "LIMIT-02": "limit_plus_one_error.code()",
            "LIMIT-03": "limit_plus_one_error.code()",
        }[row_id]
        limit_assertion = normalized_assertion(
            assertions,
            f"::core::assert_eq!({projection}, {json.dumps(expected_limit)});",
            f"{row_id}/{subcase_id} limit code",
        )
    semantic = {
        "no_mutation": comparison_assertion(
            assertions,
            "plus_one_owner_tree_before_snapshot",
            "plus_one_owner_tree_after_snapshot",
            f"{row_id}/{subcase_id} no mutation",
        ),
        "frozen_default": frozen_default_assertion,
        "exact_limit_success": normalized_assertion(
            assertions,
            "::core::assert!(exact_result.is_ok());",
            f"{row_id}/{subcase_id} exact success",
        ),
        "limit_plus_one_code": limit_assertion,
        "no_partial_report": normalized_assertion(
            assertions,
            "::core::assert!(plus_one_result.is_err());",
            f"{row_id}/{subcase_id} no partial report",
        ),
    }
    entry: dict[str, object] = {
        "result": "PASS",
        "fresh_fixture": True,
        "tests": [definition.name],
        "assertions": assertions,
        "no_mutation": True,
        "frozen_default": frozen_default,
        "exact_limit_success": True,
        "limit_plus_one_code": expected_limit,
        "no_partial_report": True,
        "semantic_assertions": semantic,
    }
    key = (row_id, subcase_id, None)
    if key in contract.MULTIFAULT_OVERLAYS:
        entry.update(multifault_fields(row_id, subcase_id, None))
    problem = contract.limit_operation_binding_problem(
        definition.body, definition.source, row_id, subcase_id, semantic
    )
    if problem is not None:
        raise RuntimeError(f"{row_id}/{subcase_id} limit body: {problem}")
    if key in contract.MULTIFAULT_OVERLAYS:
        problem = contract.multifault_evidence_problem(
            row_id, subcase_id, entry, definition.source
        )
        if problem is not None:
            raise RuntimeError(f"{row_id}/{subcase_id} limit multifault: {problem}")
    return entry


def matrix_case_entry(
    inventory: TestInventory,
    row_id: str,
    subcase_id: str | None = None,
    leaf_id: str | None = None,
) -> dict[str, object]:
    definition = inventory.resolve(row_id, subcase_id, leaf_id)
    if leaf_id is not None:
        return error_entry(row_id, subcase_id, leaf_id, definition)
    if row_id in contract.NON_GC_DURABILITY_ROWS:
        return durability_entry(row_id, definition)
    if row_id in contract.RECOVERY_SUCCESS_EXPECTATIONS:
        return recovery_success_entry(row_id, definition)
    if row_id in contract.ROW_ERROR_SPECS or row_id in contract.ERROR_CASE_SPECS:
        return error_entry(row_id, subcase_id, None, definition)
    if row_id in contract.OWNED_ENTRY_FAMILIES:
        if subcase_id is None:
            raise RuntimeError(f"{row_id} lacks an owned-entry subcase")
        return owned_entry(row_id, subcase_id, definition)
    if row_id in contract.GC_PARTIAL_REPORT_ROWS:
        if subcase_id is None:
            raise RuntimeError(f"{row_id} lacks a GC partial-report subcase")
        return gc_partial_entry(row_id, subcase_id, definition)
    if row_id == "CROSS-05":
        if subcase_id is None:
            raise RuntimeError("CROSS-05 lacks a subcase")
        return cross05_entry(row_id, subcase_id, definition)
    if row_id.startswith("LIMIT-"):
        if subcase_id is None:
            raise RuntimeError(f"{row_id} lacks a limit subcase")
        return limit_entry(row_id, subcase_id, definition)
    if row_id.startswith("GCW-"):
        return gc_witness_entry(row_id, definition)
    raise RuntimeError(f"{row_id}/{subcase_id}/{leaf_id} has no entry builder")


def build_matrix_map(inventory: TestInventory) -> dict[str, object]:
    matrix: dict[str, object] = {}
    for row_id in contract.MATRIX_IDS:
        family = contract.EVIDENCE_FAMILIES.get(row_id)
        if family is None:
            matrix[row_id] = matrix_case_entry(inventory, row_id)
            continue
        subcases: dict[str, object] = {}
        for subcase_id in family:
            if row_id in contract.GROUPED_ERROR_CASES:
                leaves = contract.GROUPED_ERROR_CASES[row_id][subcase_id]
                subcases[subcase_id] = {
                    "result": "PASS",
                    "cases": {
                        leaf_id: matrix_case_entry(
                            inventory, row_id, subcase_id, leaf_id
                        )
                        for leaf_id, _code, _variant, _chain in leaves
                    },
                }
            else:
                subcases[subcase_id] = matrix_case_entry(inventory, row_id, subcase_id)
        matrix[row_id] = {"result": "PASS", "subcases": subcases}
    return matrix


def ordered_mapped_tests(matrix: dict[str, object]) -> list[str]:
    tests: list[str] = []
    for row_id in contract.MATRIX_IDS:
        row = matrix[row_id]
        if not isinstance(row, dict):
            raise RuntimeError(f"{row_id} row is not an object")
        family = contract.EVIDENCE_FAMILIES.get(row_id)
        if family is None:
            tests.extend(row["tests"])
            continue
        subcases = row["subcases"]
        if not isinstance(subcases, dict):
            raise RuntimeError(f"{row_id} subcases are not an object")
        for subcase_id in family:
            subcase = subcases[subcase_id]
            if not isinstance(subcase, dict):
                raise RuntimeError(f"{row_id}/{subcase_id} is not an object")
            if row_id in contract.GROUPED_ERROR_CASES:
                cases = subcase["cases"]
                if not isinstance(cases, dict):
                    raise RuntimeError(f"{row_id}/{subcase_id} cases differ")
                for leaf_id, _code, _variant, _chain in contract.GROUPED_ERROR_CASES[
                    row_id
                ][subcase_id]:
                    leaf = cases[leaf_id]
                    if not isinstance(leaf, dict):
                        raise RuntimeError(
                            f"{row_id}/{subcase_id}/{leaf_id} is not an object"
                        )
                    tests.extend(leaf["tests"])
            else:
                tests.extend(subcase["tests"])
    if len(tests) != 419 or len(set(tests)) != 419:
        raise RuntimeError(
            f"matrix maps {len(tests)} tests with {len(set(tests))} unique"
        )
    return tests


def mapped_test_bodies(
    inventory: TestInventory, tests: list[str]
) -> dict[str, dict[str, str]]:
    manifest: dict[str, dict[str, str]] = {}
    for test in tests:
        definition = inventory.by_name[test]
        manifest[test] = {
            "source": definition.source,
            "qualified_test": definition.qualified,
            "body_sha256": hashlib.sha256(definition.body.encode("utf-8")).hexdigest(),
        }
    return manifest


def build_test_plan(
    inventory: TestInventory,
    matrix: dict[str, object],
    contract_set_sha256: str,
) -> dict[str, object]:
    tests = ordered_mapped_tests(matrix)
    test_bodies = mapped_test_bodies(inventory, tests)
    runtime_helpers = contract.limit_runtime_helper_body_manifest(inventory.sources)
    runtime_cases = contract.limit_runtime_case_manifest(
        matrix, test_bodies, runtime_helpers
    )
    owner_bodies = contract.limit_event_owner_body_manifest(inventory.sources)
    authority_sources = {
        relative: (contract.ROOT / relative).read_text(encoding="utf-8")
        for relative in contract.limit_production_source_paths()
    }
    control_ancestries = contract.limit_event_control_ancestry_manifest(
        authority_sources
    )
    control_ledger = contract.limit_event_control_exception_partition(
        control_ancestries, authority_sources
    )
    entry_paths = contract.limit_event_entry_call_path_manifest(authority_sources)
    entry_ledger = contract.limit_event_entry_exception_partition(
        entry_paths, authority_sources
    )
    if problem := contract.limit_exception_partition_freeze_problem(
        entry_ledger, control_ledger
    ):
        raise RuntimeError(f"exception partition freeze: {problem}")
    reconciler = contract.limit_exception_reconciler_manifest()
    shared_state = contract.limit_shared_state_authority_manifest(authority_sources)
    site_proofs = contract.limit_runtime_site_proof_manifest(
        runtime_cases, owner_bodies
    )
    observations = contract.limit_event_observation_manifest(
        matrix, test_bodies, owner_bodies
    )
    return {
        "contract": "s20-530-crash-recovery-test-plan-v1",
        "contract_set_sha256": contract_set_sha256,
        "matrix_test_map": matrix,
        "limit_runtime_helpers": runtime_helpers,
        "limit_runtime_helpers_sha256": contract.canonical_json_sha256(runtime_helpers),
        "limit_runtime_cases": runtime_cases,
        "limit_runtime_cases_sha256": contract.canonical_json_sha256(runtime_cases),
        "limit_event_owner_bodies": owner_bodies,
        "limit_event_owner_bodies_sha256": contract.canonical_json_sha256(owner_bodies),
        "limit_event_control_ancestries": control_ancestries,
        "limit_event_control_ancestries_sha256": contract.canonical_json_sha256(
            control_ancestries
        ),
        "limit_event_control_exception_ledger": control_ledger,
        "limit_event_control_exception_ledger_sha256": (
            contract.canonical_json_sha256(control_ledger)
        ),
        "limit_event_entry_call_paths": entry_paths,
        "limit_event_entry_call_paths_sha256": contract.canonical_json_sha256(
            entry_paths
        ),
        "limit_event_entry_exception_ledger": entry_ledger,
        "limit_event_entry_exception_ledger_sha256": contract.canonical_json_sha256(
            entry_ledger
        ),
        "limit_exception_reconciler": reconciler,
        "limit_exception_reconciler_sha256": contract.canonical_json_sha256(reconciler),
        "limit_shared_state_authority": shared_state,
        "limit_shared_state_authority_sha256": contract.canonical_json_sha256(
            shared_state
        ),
        "limit_runtime_site_proofs": site_proofs,
        "limit_runtime_site_proofs_sha256": contract.canonical_json_sha256(site_proofs),
        "limit_event_observations": observations,
        "limit_event_observations_sha256": contract.canonical_json_sha256(observations),
        "mapped_test_bodies": test_bodies,
        "mapped_test_bodies_sha256": contract.canonical_json_sha256(test_bodies),
    }


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--contract-set-sha256",
        help="Final frozen contract-set digest required when writing the full plan.",
    )
    parser.add_argument(
        "--matrix-only",
        action="store_true",
        help="Build and validate the 419-case matrix map without derived manifests.",
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=contract.TEST_PLAN,
        help="Full test-plan output path.",
    )
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    inventory = TestInventory()
    matrix = build_matrix_map(inventory)
    tests = ordered_mapped_tests(matrix)
    matrix_sha256 = contract.canonical_json_sha256(matrix)
    if args.matrix_only:
        print(
            "S20-530 matrix plan compiler: PASS "
            f"rows={len(matrix)} tests={len(tests)} "
            f"matrix_sha256={matrix_sha256}"
        )
        return
    digest = args.contract_set_sha256
    if digest is None or re.fullmatch(r"[0-9a-f]{64}", digest) is None:
        raise SystemExit("--contract-set-sha256 must be one lowercase SHA-256")
    plan = build_test_plan(inventory, matrix, digest)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(
        json.dumps(plan, ensure_ascii=True, indent=2) + "\n", encoding="utf-8"
    )
    print(
        "S20-530 test plan compiler: PASS "
        f"rows={len(matrix)} tests={len(tests)} "
        f"bytes={args.output.stat().st_size} "
        f"sha256={hashlib.sha256(args.output.read_bytes()).hexdigest()}"
    )


if __name__ == "__main__":
    main()
