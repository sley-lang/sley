from __future__ import annotations

import dataclasses
import hashlib
import io
import json
import os
import stat
import tarfile
import tempfile
import time
import unittest
from pathlib import Path
from unittest import mock

from bench.legacy import runner as runner_module
from bench.legacy.runner import (
    LegacyArtifactContract,
    LegacyErrorCode,
    LegacyRunnerError,
    _copy_pinned_artifact,
    record_smoke_evidence,
    run_version_smoke,
    staged_frozen_artifact,
    verify_frozen_artifact,
)


TOP = "synthetic-sley-1.2.0-linux-x86_64"
RELEASE = "1.2.0"
COMMIT = "1" * 40


def _digest(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def _manifest(payload_files: dict[str, tuple[bytes, int]]) -> tuple[dict, str]:
    records = [
        {
            "digest": f"sha256:{_digest(payload)}",
            "mode": f"{mode:04o}",
            "path": path,
            "size": len(payload),
        }
        for path, (payload, mode) in sorted(payload_files.items())
    ]
    tree_digest = "sha256:" + _digest(
        json.dumps(records, sort_keys=True, separators=(",", ":")).encode()
    )
    manifest = {
        "schema": "sley.release.manifest.v1",
        "artifact_id": TOP,
        "release": RELEASE,
        "status": "release_candidate",
        "source": {
            "commit": COMMIT,
            "dirty": False,
            "payload_tree_digest": tree_digest,
        },
        "authority": {
            "mode": "local_release_candidate_only",
            "publication_authorized": False,
            "signing_status": "unsigned",
            "tag_authorized": False,
            "upload_authorized": False,
        },
        "platform": {
            "architecture": "x86_64",
            "archive_format": "tar.gz",
            "os": "linux",
            "support_status": "supported",
        },
        "metadata": {
            "license_inventory": "release/licenses.json",
            "manifest": "release/manifest.json",
            "sbom": "release/sbom.spdx.json",
        },
        "toolchain": {"sley_version": "sley 1.2.0"},
        "payload": {
            "file_count": len(records),
            "files": records,
            "inventory_scope": "tracked_release_payload_excluding_release_metadata",
            "total_bytes": sum(record["size"] for record in records),
        },
    }
    return manifest, tree_digest


def _add_directory(archive: tarfile.TarFile, name: str) -> None:
    member = tarfile.TarInfo(name + "/")
    member.type = tarfile.DIRTYPE
    member.mode = 0o755
    member.mtime = 0
    archive.addfile(member)


def _add_file(archive: tarfile.TarFile, name: str, payload: bytes, mode: int) -> None:
    member = tarfile.TarInfo(name)
    member.size = len(payload)
    member.mode = mode
    member.mtime = 0
    archive.addfile(member, io.BytesIO(payload))


def build_artifact(
    path: Path,
    script: bytes,
    *,
    corrupt_payload: bool = False,
    unsafe_kind: str | None = None,
    raw_manifest: bytes | None = None,
    patch_manifest=None,
    extra_payload_files: dict[str, tuple[bytes, int]] | None = None,
    extra_archive_files: dict[str, tuple[bytes, int]] | None = None,
    archive_payload_overrides: dict[str, bytes] | None = None,
    mode_overrides: dict[str, int] | None = None,
    archive_prefix: str | None = None,
    omit_manifest: bool = False,
    skip_directories: tuple[str, ...] = (),
    contract_overrides: dict | None = None,
) -> LegacyArtifactContract:
    prefix = archive_prefix or TOP
    payload_files = {
        "bin/sley": (script, 0o755),
        "fixture.txt": (b"frozen fixture\n", 0o644),
    }
    if extra_payload_files:
        payload_files.update(extra_payload_files)
    manifest, tree_digest = _manifest(payload_files)
    if patch_manifest is not None:
        patch_manifest(manifest)
    if raw_manifest is not None:
        manifest_bytes = raw_manifest
    else:
        manifest_bytes = json.dumps(manifest, sort_keys=True, indent=2).encode() + b"\n"
    archive_payloads = {
        "bin/sley": (script, 0o755),
        "fixture.txt": (
            b"corrupt fixture\n" if corrupt_payload else b"frozen fixture\n",
            0o644,
        ),
    }
    if extra_payload_files:
        archive_payloads.update(extra_payload_files)
    if archive_payload_overrides:
        for relative, payload in archive_payload_overrides.items():
            mode = archive_payloads[relative][1]
            archive_payloads[relative] = (payload, mode)
    if mode_overrides:
        for relative, mode in mode_overrides.items():
            archive_payloads[relative] = (archive_payloads[relative][0], mode)
    metadata = {}
    if not omit_manifest:
        metadata["release/manifest.json"] = (manifest_bytes, 0o644)
    metadata["release/licenses.json"] = (b"{}\n", 0o644)
    metadata["release/sbom.spdx.json"] = (b"{}\n", 0o644)
    if extra_archive_files:
        archive_payloads.update(extra_archive_files)
    with tarfile.open(path, "w:gz", format=tarfile.PAX_FORMAT) as archive:
        for directory in (prefix, f"{prefix}/bin", f"{prefix}/release"):
            if directory in skip_directories:
                continue
            _add_directory(archive, directory)
        for relative, (payload, mode) in sorted(archive_payloads.items()):
            _add_file(archive, f"{prefix}/{relative}", payload, mode)
        for relative, (payload, mode) in sorted(metadata.items()):
            _add_file(archive, f"{prefix}/{relative}", payload, mode)
        if unsafe_kind == "traversal":
            _add_file(archive, f"{prefix}/../escape", b"escape\n", 0o644)
        elif unsafe_kind == "symlink":
            member = tarfile.TarInfo(f"{prefix}/escape-link")
            member.type = tarfile.SYMTYPE
            member.linkname = "/tmp/escape"
            member.mode = 0o777
            archive.addfile(member)
        elif unsafe_kind == "duplicate":
            _add_file(archive, f"{prefix}/fixture.txt", b"duplicate\n", 0o644)
        elif unsafe_kind == "hardlink":
            member = tarfile.TarInfo(f"{prefix}/escape-hard")
            member.type = tarfile.LNKTYPE
            member.linkname = f"{prefix}/fixture.txt"
            member.mode = 0o644
            archive.addfile(member)
        elif unsafe_kind == "fifo":
            member = tarfile.TarInfo(f"{prefix}/escape-fifo")
            member.type = tarfile.FIFOTYPE
            member.mode = 0o644
            archive.addfile(member)
        elif unsafe_kind == "device":
            member = tarfile.TarInfo(f"{prefix}/escape-dev")
            member.type = tarfile.CHRTYPE
            member.mode = 0o644
            member.devmajor = 1
            member.devminor = 5
            archive.addfile(member)
        elif unsafe_kind == "absolute":
            _add_file(archive, "/absolute-escape", b"escape\n", 0o644)
        elif unsafe_kind == "setuid":
            _add_file(archive, f"{prefix}/escape-suid", b"escape\n", 0o4755)
    artifact_bytes = path.read_bytes()
    contract = LegacyArtifactContract(
        artifact_sha256=_digest(artifact_bytes),
        artifact_size_bytes=len(artifact_bytes),
        top_level_directory=TOP,
        release=RELEASE,
        source_commit=COMMIT,
        artifact_id=TOP,
        expected_version_output="sley 1.2.0",
        payload_tree_digest=tree_digest,
        expected_sley_digest=_digest(script),
        max_archive_members=32,
        max_regular_files=16,
        max_total_regular_bytes=256 * 1024,
        max_member_bytes=128 * 1024,
        max_manifest_bytes=64 * 1024,
    )
    if contract_overrides:
        contract = dataclasses.replace(contract, **contract_overrides)
    return contract


SUCCESS_SCRIPT = b"#!/bin/sh\nprintf 'sley 1.2.0\\n'\n"


class LegacyRunnerTests(unittest.TestCase):
    def test_valid_artifact_is_verified_stage_write_bits_removed_and_smoked(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            artifact = Path(temporary) / "artifact.tar.gz"
            contract = build_artifact(artifact, SUCCESS_SCRIPT)
            verified = verify_frozen_artifact(artifact, contract)
            self.assertTrue(verified.report()["payload_inventory_verified"])
            self.assertEqual(verified.payload_file_count, 2)

            with staged_frozen_artifact(artifact, contract) as stage:
                stage_root = stage.root
                self.assertFalse(stat.S_IMODE(stage.root.stat().st_mode) & 0o222)
                self.assertFalse(
                    stat.S_IMODE((stage.root / "fixture.txt").stat().st_mode) & 0o222
                )
                self.assertTrue(os.access(stage.scratch, os.W_OK))
            self.assertFalse(stage_root.exists())

            report = run_version_smoke(artifact, contract, timeout_seconds=2.0)
            self.assertTrue(report["success"])
            self.assertEqual(report["execution"]["return_code"], 0)
            self.assertEqual(report["benchmark_trials_executed"], 0)

    def test_outer_identity_drift_fails_before_archive_use(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            artifact = Path(temporary) / "artifact.tar.gz"
            contract = build_artifact(artifact, SUCCESS_SCRIPT)
            with artifact.open("ab") as output:
                output.write(b"drift")
            with self.assertRaises(LegacyRunnerError) as caught:
                verify_frozen_artifact(artifact, contract)
            self.assertEqual(
                caught.exception.code, LegacyErrorCode.ARTIFACT_IDENTITY_MISMATCH
            )

    def test_traversal_links_and_duplicates_fail_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            for unsafe_kind in ("traversal", "symlink", "duplicate"):
                with self.subTest(unsafe_kind=unsafe_kind):
                    artifact = Path(temporary) / f"{unsafe_kind}.tar.gz"
                    contract = build_artifact(
                        artifact, SUCCESS_SCRIPT, unsafe_kind=unsafe_kind
                    )
                    with self.assertRaises(LegacyRunnerError) as caught:
                        verify_frozen_artifact(artifact, contract)
                    self.assertEqual(
                        caught.exception.code, LegacyErrorCode.ARCHIVE_MEMBER_UNSAFE
                    )

    def test_payload_tamper_fails_manifest_inventory_check(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            artifact = Path(temporary) / "artifact.tar.gz"
            contract = build_artifact(artifact, SUCCESS_SCRIPT, corrupt_payload=True)
            with self.assertRaises(LegacyRunnerError) as caught:
                verify_frozen_artifact(artifact, contract)
            self.assertEqual(caught.exception.code, LegacyErrorCode.PAYLOAD_MISMATCH)

    def test_timeout_is_returned_as_retained_failure(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            artifact = Path(temporary) / "artifact.tar.gz"
            contract = build_artifact(artifact, b"#!/bin/sh\nsleep 2\n")
            report = run_version_smoke(artifact, contract, timeout_seconds=0.05)
            self.assertFalse(report["success"])
            self.assertEqual(report["status"], "timeout")
            self.assertEqual(report["failure_code"], "LEGACY_COMMAND_TIMEOUT")
            self.assertEqual(report["execution"]["return_code"], -9)

    def test_nonzero_exit_and_stderr_are_retained(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            artifact = Path(temporary) / "artifact.tar.gz"
            script = b"#!/bin/sh\nprintf 'frozen failure\\n' >&2\nexit 7\n"
            contract = build_artifact(artifact, script)
            report = run_version_smoke(artifact, contract, timeout_seconds=2.0)
            self.assertFalse(report["success"])
            self.assertEqual(report["status"], "failed")
            self.assertEqual(report["execution"]["return_code"], 7)
            self.assertEqual(report["execution"]["stderr"]["byte_count"], 15)
            self.assertFalse(report["execution"]["stderr"]["truncated"])

    def test_output_limit_kills_and_retains_prefix(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            artifact = Path(temporary) / "artifact.tar.gz"
            contract = build_artifact(artifact, b"#!/bin/sh\nyes X\n")
            report = run_version_smoke(
                artifact,
                contract,
                timeout_seconds=2.0,
                output_limit_bytes=1_024,
            )
            self.assertFalse(report["success"])
            self.assertEqual(report["status"], "output_limit")
            self.assertEqual(report["failure_code"], "LEGACY_COMMAND_OUTPUT_LIMIT")
            self.assertEqual(
                report["execution"]["stdout"]["retained_prefix_bytes"], 1_024
            )
            self.assertTrue(report["execution"]["stdout"]["truncated"])

    def test_nonfinite_timeout_fails_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            artifact = Path(temporary) / "artifact.tar.gz"
            contract = build_artifact(artifact, SUCCESS_SCRIPT)
            report = run_version_smoke(
                artifact,
                contract,
                timeout_seconds=float("nan"),
            )
            self.assertFalse(report["success"])
            self.assertEqual(report["status"], "harness_failure")
            self.assertEqual(report["failure_code"], "LEGACY_COMMAND_NOT_ALLOWED")

    def test_evidence_records_are_create_only(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary) / "evidence"
            first = {
                "attempt_started_at_utc": "2026-08-27T12:00:00Z",
                "status": "timeout",
            }
            second = {
                "attempt_started_at_utc": "2026-08-27T12:00:01Z",
                "status": "completed",
            }
            first_path = record_smoke_evidence(directory, first)
            first_bytes = first_path.read_bytes()
            second_path = record_smoke_evidence(directory, second)
            self.assertNotEqual(first_path, second_path)
            self.assertEqual(first_path.read_bytes(), first_bytes)
            self.assertEqual(len(list(directory.glob("*.json"))), 2)
            with self.assertRaises(LegacyRunnerError) as caught:
                record_smoke_evidence(directory, first)
            self.assertEqual(
                caught.exception.code, LegacyErrorCode.EVIDENCE_WRITE_FAILED
            )


    def test_error_code_enum_is_stable_and_complete(self) -> None:
        self.assertEqual(
            [(code.name, code.value) for code in LegacyErrorCode],
            [
                ("ARTIFACT_MISSING", 60_000),
                ("ARTIFACT_IDENTITY_MISMATCH", 60_001),
                ("ARCHIVE_INVALID", 60_002),
                ("ARCHIVE_MEMBER_UNSAFE", 60_003),
                ("ARCHIVE_LIMIT_EXCEEDED", 60_004),
                ("MANIFEST_INVALID", 60_005),
                ("MANIFEST_IDENTITY_MISMATCH", 60_006),
                ("PAYLOAD_MISMATCH", 60_007),
                ("STAGING_FAILED", 60_008),
                ("COMMAND_NOT_ALLOWED", 60_009),
                ("COMMAND_TIMEOUT", 60_010),
                ("COMMAND_OUTPUT_LIMIT", 60_011),
                ("COMMAND_FAILED", 60_012),
                ("EVIDENCE_WRITE_FAILED", 60_013),
                ("INTERNAL_INVARIANT", 60_014),
            ],
        )

    def test_missing_artifact_path_reports_missing(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            artifact = Path(temporary) / "artifact.tar.gz"
            contract = build_artifact(artifact, SUCCESS_SCRIPT)
            missing = Path(temporary) / "absent.tar.gz"
            with self.assertRaises(LegacyRunnerError) as caught:
                verify_frozen_artifact(missing, contract)
            self.assertEqual(
                caught.exception.code, LegacyErrorCode.ARTIFACT_MISSING
            )

    def test_garbage_archive_reports_invalid(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            artifact = Path(temporary) / "artifact.tar.gz"
            garbage = b"not a gzip stream" * 64
            artifact.write_bytes(garbage)
            contract = LegacyArtifactContract(
                artifact_sha256=_digest(garbage),
                artifact_size_bytes=len(garbage),
                top_level_directory=TOP,
                release=RELEASE,
                source_commit=COMMIT,
                artifact_id=TOP,
                expected_version_output="sley 1.2.0",
                payload_tree_digest="sha256:" + "0" * 64,
                expected_sley_digest=_digest(SUCCESS_SCRIPT),
                max_archive_members=32,
                max_regular_files=16,
                max_total_regular_bytes=256 * 1024,
                max_member_bytes=128 * 1024,
                max_manifest_bytes=64 * 1024,
            )
            with self.assertRaises(LegacyRunnerError) as caught:
                verify_frozen_artifact(artifact, contract)
            self.assertEqual(
                caught.exception.code, LegacyErrorCode.ARCHIVE_INVALID
            )

    def test_archive_ceilings_fail_closed(self) -> None:
        cases = {
            "member_bytes": (
                {"big.bin": (b"X" * (200 * 1024), 0o644)},
                None,
            ),
            "total_bytes": (
                {
                    f"bulk-{index}.bin": (b"Y" * (100 * 1024), 0o644)
                    for index in range(3)
                },
                None,
            ),
            # Raised file ceiling so the member-count ceiling fires first
            # (default ceilings trip the file count at member #20).
            "member_count": (
                {
                    f"file-{index:02d}.txt": (b"filler\n", 0o644)
                    for index in range(30)
                },
                {"max_regular_files": 64},
            ),
            "file_count": (
                {
                    f"file-{index:02d}.txt": (b"filler\n", 0o644)
                    for index in range(12)
                },
                None,
            ),
        }
        for name, (extra, overrides) in cases.items():
            with self.subTest(ceiling=name):
                with tempfile.TemporaryDirectory() as temporary:
                    artifact = Path(temporary) / "artifact.tar.gz"
                    contract = build_artifact(
                        artifact,
                        SUCCESS_SCRIPT,
                        extra_payload_files=extra,
                        contract_overrides=overrides,
                    )
                    with self.assertRaises(LegacyRunnerError) as caught:
                        verify_frozen_artifact(artifact, contract)
                    self.assertEqual(
                        caught.exception.code,
                        LegacyErrorCode.ARCHIVE_LIMIT_EXCEEDED,
                    )

    def test_oversized_manifest_fails_closed(self) -> None:
        def pad(manifest: dict) -> None:
            manifest["padding"] = "x" * (70 * 1024)

        with tempfile.TemporaryDirectory() as temporary:
            artifact = Path(temporary) / "artifact.tar.gz"
            contract = build_artifact(artifact, SUCCESS_SCRIPT, patch_manifest=pad)
            with self.assertRaises(LegacyRunnerError) as caught:
                verify_frozen_artifact(artifact, contract)
            self.assertEqual(
                caught.exception.code, LegacyErrorCode.ARCHIVE_LIMIT_EXCEEDED
            )

    def test_malformed_and_duplicate_key_manifests_fail_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            variants = {
                "not_json": b"this is not json",
                "not_object": b"[1, 2, 3]\n",
                "duplicate_key": b'{"schema": "a", "schema": "b"}\n',
            }
            for name, raw in variants.items():
                with self.subTest(variant=name):
                    artifact = Path(temporary) / f"{name}.tar.gz"
                    contract = build_artifact(
                        artifact, SUCCESS_SCRIPT, raw_manifest=raw
                    )
                    with self.assertRaises(LegacyRunnerError) as caught:
                        verify_frozen_artifact(artifact, contract)
                    self.assertEqual(
                        caught.exception.code, LegacyErrorCode.MANIFEST_INVALID
                    )

    def test_manifest_missing_field_fails_closed(self) -> None:
        def drop_source(manifest: dict) -> None:
            del manifest["source"]

        with tempfile.TemporaryDirectory() as temporary:
            artifact = Path(temporary) / "artifact.tar.gz"
            contract = build_artifact(
                artifact, SUCCESS_SCRIPT, patch_manifest=drop_source
            )
            with self.assertRaises(LegacyRunnerError) as caught:
                verify_frozen_artifact(artifact, contract)
            self.assertEqual(
                caught.exception.code, LegacyErrorCode.MANIFEST_INVALID
            )

    def test_missing_manifest_file_fails_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            artifact = Path(temporary) / "artifact.tar.gz"
            contract = build_artifact(artifact, SUCCESS_SCRIPT, omit_manifest=True)
            with self.assertRaises(LegacyRunnerError) as caught:
                verify_frozen_artifact(artifact, contract)
            self.assertEqual(
                caught.exception.code, LegacyErrorCode.MANIFEST_INVALID
            )

    def test_authority_drift_and_tree_digest_drift_fail_closed(self) -> None:
        def authorize(manifest: dict) -> None:
            manifest["authority"]["publication_authorized"] = True

        def drift_tree(manifest: dict) -> None:
            manifest["source"]["payload_tree_digest"] = "sha256:" + "0" * 64

        def drift_counts(manifest: dict) -> None:
            manifest["payload"]["file_count"] += 1

        variants = {
            "authority": (authorize, LegacyErrorCode.MANIFEST_IDENTITY_MISMATCH),
            "tree_digest": (drift_tree, LegacyErrorCode.MANIFEST_IDENTITY_MISMATCH),
            # A self-inconsistent count is INVALID (payload counts must
            # agree with the record list); MISMATCH needs a contract that
            # pins the expectation, covered by the two variants above.
            "file_count": (drift_counts, LegacyErrorCode.MANIFEST_INVALID),
        }
        for name, (patch, expected) in variants.items():
            with self.subTest(drift=name):
                with tempfile.TemporaryDirectory() as temporary:
                    artifact = Path(temporary) / f"{name}.tar.gz"
                    contract = build_artifact(
                        artifact, SUCCESS_SCRIPT, patch_manifest=patch
                    )
                    with self.assertRaises(LegacyRunnerError) as caught:
                        verify_frozen_artifact(artifact, contract)
                    self.assertEqual(caught.exception.code, expected)

    def test_contamination_member_kinds_fail_closed(self) -> None:
        for unsafe_kind in (
            "hardlink",
            "fifo",
            "device",
            "absolute",
            "setuid",
        ):
            with self.subTest(unsafe_kind=unsafe_kind):
                with tempfile.TemporaryDirectory() as temporary:
                    artifact = Path(temporary) / f"{unsafe_kind}.tar.gz"
                    contract = build_artifact(
                        artifact, SUCCESS_SCRIPT, unsafe_kind=unsafe_kind
                    )
                    with self.assertRaises(LegacyRunnerError) as caught:
                        verify_frozen_artifact(artifact, contract)
                    self.assertEqual(
                        caught.exception.code, LegacyErrorCode.ARCHIVE_MEMBER_UNSAFE
                    )

    def test_wrong_top_and_directory_drift_fail_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            artifact = Path(temporary) / "wrong-top.tar.gz"
            contract = build_artifact(
                artifact, SUCCESS_SCRIPT, archive_prefix="intruder-top"
            )
            with self.assertRaises(LegacyRunnerError) as caught:
                verify_frozen_artifact(artifact, contract)
            self.assertEqual(
                caught.exception.code, LegacyErrorCode.ARCHIVE_MEMBER_UNSAFE
            )
            drifted = Path(temporary) / "drifted.tar.gz"
            drift_contract = build_artifact(
                drifted, SUCCESS_SCRIPT, skip_directories=(f"{TOP}/bin",)
            )
            with self.assertRaises(LegacyRunnerError) as caught:
                verify_frozen_artifact(drifted, drift_contract)
            self.assertEqual(
                caught.exception.code, LegacyErrorCode.ARCHIVE_MEMBER_UNSAFE
            )

    def test_unmanifested_payload_and_sley_identity_fail_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            extra = Path(temporary) / "extra.tar.gz"
            contract = build_artifact(
                extra,
                SUCCESS_SCRIPT,
                extra_archive_files={"stowaway.txt": (b"hidden\n", 0o644)},
            )
            with self.assertRaises(LegacyRunnerError) as caught:
                verify_frozen_artifact(extra, contract)
            self.assertEqual(
                caught.exception.code, LegacyErrorCode.PAYLOAD_MISMATCH
            )
            # Manifest-consistent non-executable bin/sley reaches the
            # contract-level executable check (not the manifest equality).
            nonexec = Path(temporary) / "nonexec.tar.gz"
            nonexec_contract = build_artifact(
                nonexec,
                SUCCESS_SCRIPT,
                extra_payload_files={"bin/sley": (SUCCESS_SCRIPT, 0o644)},
            )
            with self.assertRaises(LegacyRunnerError) as caught:
                verify_frozen_artifact(nonexec, nonexec_contract)
            self.assertEqual(
                caught.exception.code, LegacyErrorCode.PAYLOAD_MISMATCH
            )
            # Manifest-consistent foreign script reaches the contract-level
            # digest check: the manifest matches the archive, but the
            # contract pins the expected script.
            other_script = b"#!/bin/sh\nprintf 'sley 9.9.9\\n'\n"
            drifted = Path(temporary) / "drifted-sley.tar.gz"
            drift_contract = build_artifact(
                drifted,
                SUCCESS_SCRIPT,
                extra_payload_files={"bin/sley": (other_script, 0o755)},
            )
            with self.assertRaises(LegacyRunnerError) as caught:
                verify_frozen_artifact(drifted, drift_contract)
            self.assertEqual(
                caught.exception.code, LegacyErrorCode.PAYLOAD_MISMATCH
            )

    def test_invalid_contract_fails_invariant(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            artifact = Path(temporary) / "artifact.tar.gz"
            contract = build_artifact(artifact, SUCCESS_SCRIPT)
            bad_digest = LegacyArtifactContract(
                **{**contract.__dict__, "artifact_sha256": "not-a-digest"},
            )
            with self.assertRaises(LegacyRunnerError) as caught:
                verify_frozen_artifact(artifact, bad_digest)
            self.assertEqual(
                caught.exception.code, LegacyErrorCode.INTERNAL_INVARIANT
            )
            bad_size = LegacyArtifactContract(
                **{**contract.__dict__, "artifact_size_bytes": 0},
            )
            with self.assertRaises(LegacyRunnerError) as caught:
                verify_frozen_artifact(artifact, bad_size)
            self.assertEqual(
                caught.exception.code, LegacyErrorCode.INTERNAL_INVARIANT
            )

    def test_staging_copy_failure_reports_staging_failed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            artifact = Path(temporary) / "artifact.tar.gz"
            contract = build_artifact(artifact, SUCCESS_SCRIPT)
            destination = Path(temporary) / "no-such-dir" / "copy.tar.gz"
            with self.assertRaises(LegacyRunnerError) as caught:
                _copy_pinned_artifact(artifact, destination, contract)
            self.assertEqual(
                caught.exception.code, LegacyErrorCode.STAGING_FAILED
            )

    def test_command_failed_reports_symbol(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            artifact = Path(temporary) / "artifact.tar.gz"
            script = b"#!/bin/sh\nprintf 'frozen failure\\n' >&2\nexit 7\n"
            contract = build_artifact(artifact, script)
            report = run_version_smoke(artifact, contract, timeout_seconds=2.0)
            self.assertFalse(report["success"])
            self.assertEqual(report["status"], "failed")
            self.assertEqual(report["failure_code"], "LEGACY_COMMAND_FAILED")

    def test_hung_child_that_closed_fds_is_a_timeout(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            artifact = Path(temporary) / "artifact.tar.gz"
            script = b"#!/bin/sh\nexec >&- 2>&-\nsleep 5\n"
            contract = build_artifact(artifact, script)
            started = time.monotonic()
            report = run_version_smoke(artifact, contract, timeout_seconds=0.5)
            elapsed = time.monotonic() - started
            self.assertFalse(report["success"])
            self.assertEqual(report["status"], "timeout")
            self.assertEqual(report["failure_code"], "LEGACY_COMMAND_TIMEOUT")
            self.assertLess(elapsed, 10.0)

    def test_escaped_grandchild_does_not_spin_the_drain(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            artifact = Path(temporary) / "artifact.tar.gz"
            script = b"#!/bin/sh\nsetsid sleep 5\n"
            contract = build_artifact(artifact, script)
            started = time.monotonic()
            with mock.patch.object(runner_module, "DRAIN_GRACE_SECONDS", 0.0):
                report = run_version_smoke(
                    artifact, contract, timeout_seconds=0.3
                )
            elapsed = time.monotonic() - started
            # The pre-fix loop spins on the orphaned pipe until the
            # grandchild exits (~5 s); the bounded drain abandons it.
            self.assertLess(elapsed, 3.0)
            self.assertFalse(report["success"])
            self.assertEqual(report["status"], "timeout")
            self.assertEqual(report["failure_code"], "LEGACY_COMMAND_TIMEOUT")

    def test_mode_hardening_failure_reaches_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            artifact = Path(temporary) / "artifact.tar.gz"
            contract = build_artifact(artifact, SUCCESS_SCRIPT)

            def fail_chmod(path, mode):
                raise OSError(28, "No space left on device")

            with mock.patch.object(os, "chmod", fail_chmod):
                with self.assertRaises(LegacyRunnerError) as caught:
                    with staged_frozen_artifact(artifact, contract):
                        pass
                self.assertEqual(
                    caught.exception.code, LegacyErrorCode.STAGING_FAILED
                )
                report = run_version_smoke(
                    artifact, contract, timeout_seconds=2.0
                )
            self.assertFalse(report["success"])
            self.assertEqual(report["status"], "harness_failure")
            self.assertEqual(report["failure_code"], "LEGACY_STAGING_FAILED")


if __name__ == "__main__":
    unittest.main()
