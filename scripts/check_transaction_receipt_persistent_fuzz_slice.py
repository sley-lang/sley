#!/usr/bin/env python3
"""Drift check for the S20-390 transaction/receipt persistent target."""

from __future__ import annotations

import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
TARGET = ROOT / "fuzz/targets/transaction_receipt.rs"
MANIFEST = ROOT / "fuzz/Cargo.toml"
RUNNER = ROOT / "scripts/run_transaction_receipt_persistent_fuzz.py"
MAKEFILE = ROOT / "Makefile"
ACCEPTED = ROOT / "conformance/transaction-receipt/v1/accepted.json"
REJECTED = ROOT / "conformance/transaction-receipt/v1/rejected.json"
GENERATOR = ROOT / "scripts/generate_transaction_receipt_fixtures.py"


def main() -> int:
    problems: list[str] = []
    for path in (TARGET, MANIFEST, RUNNER, MAKEFILE, ACCEPTED, REJECTED, GENERATOR):
        if not path.is_file():
            problems.append(f"missing:{path.relative_to(ROOT)}")
    if problems:
        raise SystemExit("\n".join(problems))

    target = TARGET.read_text(encoding="utf-8")
    for marker in (
        "LLVMFuzzerTestOneInput",
        "import_transaction(input)",
        "import_transaction_receipt(input)",
        "TransactionId::derive(&first.preimage)",
        "ReceiptId::derive(&first.preimage)",
        "TransactionKind::TrustedGenesis",
        "TransactionKind::OrdinaryCandidate",
    ):
        if marker not in target:
            problems.append(f"target-missing:{marker}")

    manifest = MANIFEST.read_text(encoding="utf-8")
    for marker in (
        'name = "transaction_receipt"',
        'path = "targets/transaction_receipt.rs"',
        'sley-txn = { path = "../crates/sley-txn" }',
    ):
        if marker not in manifest:
            problems.append(f"manifest-missing:{marker}")

    runner = RUNNER.read_text(encoding="utf-8")
    for marker in (
        "libclang_rt.fuzzer-x86_64.a",
        "nightly-2026-02-27",
        '"TRANSACTION_RECEIPT_IMPORT_AND_CROSS_BINDING_NO_COMMIT"',
        '"commit_authority": False',
        '"runtime_mutation": False',
        'bytes.fromhex(vector["transaction_hex"])',
        'bytes.fromhex(vector["receipt_hex"])',
    ):
        if marker not in runner:
            problems.append(f"runner-missing:{marker}")
    # Repair round 7 uniform harness markers (REQ-06 wave): locked build,
    # host-config owner-lib instrumentation, corpus-coverage gate, executed and
    # coverage proof, crash minimization, and persistent corpus discipline.
    for marker in [
        "--locked",
        "-Zhost-config",
        "executed_runs",
        "sync_seed_corpus",
        "minimize_crashes",
        "owner_lib_sancov_symbols",
        "corpus_persistent",
        "SLEY_FUZZ_CC",
        "-minimize_crash=1",
        "OWNER_RLIB",
        "corpus_file_count",
        "coverage_counters",
        "trace-compares",
        "must cover the corpus",
    ]:
        if marker not in runner:
            problems.append(f"runner-missing:{marker}")

    makefile = MAKEFILE.read_text(encoding="utf-8")
    for marker in (
        "transaction-receipt-persistent-fuzz-smoke:",
        "python3 scripts/generate_transaction_receipt_fixtures.py --check",
        "python3 scripts/check_transaction_receipt_persistent_fuzz_slice.py",
        "python3 scripts/run_transaction_receipt_persistent_fuzz.py",
    ):
        if marker not in makefile:
            problems.append(f"makefile-missing:{marker}")

    accepted = json.loads(ACCEPTED.read_text(encoding="utf-8"))
    rejected = json.loads(REJECTED.read_text(encoding="utf-8"))
    if [value.get("kind") for value in accepted.get("vectors", [])] != [
        "GENESIS",
        "ORDINARY",
        "ORDINARY_EXTENDED",
    ]:
        problems.append("accepted-kind-coverage-drift")
    if len(rejected.get("mutations", [])) != 10:
        problems.append("rejected-vector-count-drift")
    if {value.get("target") for value in rejected.get("mutations", [])} != {
        "transaction",
        "receipt",
    }:
        problems.append("rejected-target-coverage-drift")
    if not any(
        value.get("operation") == "provided-inventory-mismatch"
        and value.get("expected_code") == "TXN_OBJECT_INVENTORY_MISMATCH"
        for value in rejected.get("mutations", [])
    ):
        problems.append("manifest-length-rejection-missing")
    if not any(
        value.get("operation") == "provided-genesis-profile"
        and value.get("target") == "transaction"
        and value.get("expected_code") == "TXN_FIELD_SHAPE"
        for value in rejected.get("mutations", [])
    ):
        problems.append("genesis-profile-rejection-missing")

    generator = GENERATOR.read_text(encoding="utf-8")
    for marker in (
        "emit_transaction_receipt_vectors_for_fixture_refresh",
        'parser.add_argument(\n        "--check"',
        '"ORDINARY_EXTENDED",',
        '"genesis-extended-profile",',
        '"generator": "scripts/generate_transaction_receipt_fixtures.py"',
    ):
        if marker not in generator:
            problems.append(f"generator-missing:{marker}")

    result = {
        "commit_authority": False,
        "contract": "s20-390-transaction-receipt-persistent-libfuzzer-v1",
        "problems": problems,
        "result": "PASS" if not problems else "FAIL",
        "runtime_mutation": False,
        "scope": "TRANSACTION_RECEIPT_IMPORT_AND_CROSS_BINDING_NO_COMMIT",
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0 if not problems else 1


if __name__ == "__main__":
    raise SystemExit(main())
