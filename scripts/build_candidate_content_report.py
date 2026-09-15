#!/usr/bin/env python3
"""Record artifact member and content checks against an attested candidate."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import sys
import tarfile
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import build_release_candidate as packaging
import build_reproducibility_report as reproducibility

ROOT = Path(__file__).resolve().parents[1]
REPORT = ROOT / "evidence/release/candidate-content-checks.json"
REPRO = ROOT / "evidence/release/reproducibility-report.json"
CONTRACT = "sley2.candidate-content-checks.v1"


def canonical(value: object) -> str:
    return json.dumps(value, indent=2, sort_keys=True) + "\n"


def build_report(artifact: Path, attestation: dict) -> dict:
    reproducibility.validate_attestation(attestation)
    identity = {
        key: attestation[key]
        for key in ("commit", "artifact_sha256", "manifest_digest", "artifact_size_bytes")
    }
    if (
        artifact.stat().st_size != identity["artifact_size_bytes"]
        or packaging.sha256_file(artifact) != identity["artifact_sha256"]
    ):
        raise ValueError("artifact bytes differ from the selected attestation")
    listing = subprocess.run(
        ["git", "ls-tree", "-r", "--name-only", identity["commit"], "--", *packaging.CONFORMANCE_SUBSET],
        cwd=ROOT, text=True, capture_output=True,
    )
    if listing.returncode:
        raise ValueError("selected candidate commit is unavailable")
    fixtures = listing.stdout.splitlines()
    expected = {"bin/sley", "MANIFEST.json", "SBOM.json", "LICENSES.json", "LICENSE", "NOTICE", "demo/run_demo.py", *fixtures}
    with tarfile.open(artifact, "r:gz") as archive:
        members = [member.name for member in archive.getmembers() if member.isfile()]
        if len(members) != len(set(members)) or set(members) != {f"{packaging.ARTIFACT_STEM}/{path}" for path in expected}:
            raise ValueError("artifact member set differs from S20-720 contents")
        expected_directories = {packaging.ARTIFACT_STEM}
        for path in expected:
            expected_directories.update(
                f"{packaging.ARTIFACT_STEM}/{parent}"
                for parent in Path(path).parents if str(parent) != "."
            )
        directories = {member.name.rstrip("/") for member in archive.getmembers() if member.isdir()}
        if directories != expected_directories:
            raise ValueError("artifact directory set differs from S20-720 contents")
        if any(not (member.isfile() or member.isdir()) for member in archive.getmembers()):
            raise ValueError("artifact contains a non-file, non-directory member")
    with tempfile.TemporaryDirectory(prefix="sley-content-check-") as temporary:
        stage = packaging.unpack(artifact, Path(temporary) / "unpacked")
        manifest = json.loads((stage / "MANIFEST.json").read_text(encoding="utf-8"))
        packaging.verify_manifest(stage, manifest)
        if manifest["commit"] != identity["commit"] or manifest["manifest_digest"] != identity["manifest_digest"]:
            raise ValueError("manifest differs from the selected attestation")
        findings = packaging.scan_forbidden_content(
            stage, (str(ROOT), "/home/", "/home-remapped", Path.home().name)
        )
    report = {
        "contract": CONTRACT,
        **identity,
        "checks": {"manifest": True, "forbidden_content": not findings},
        "result": "FAIL" if findings else "PASS",
    }
    report["report_digest"] = hashlib.sha256(canonical(report).encode("utf-8")).hexdigest()
    return report


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    arguments = parser.parse_args()
    try:
        repro = json.loads(REPRO.read_text(encoding="utf-8"))
        attestation = reproducibility.select_attestation(repro)
        if attestation is None:
            raise ValueError("no unique admissible candidate attestation")
        report = build_report(ROOT / "dist" / packaging.ARTIFACT_NAME, attestation)
        text = canonical(report)
        if arguments.check:
            if not REPORT.exists() or REPORT.read_text(encoding="utf-8") != text:
                raise ValueError("tracked candidate content checks differ from the artifact")
        else:
            REPORT.parent.mkdir(parents=True, exist_ok=True)
            REPORT.write_text(text, encoding="utf-8")
        print(json.dumps({"contract": CONTRACT, "result": report["result"]}))
        return 0 if report["result"] == "PASS" else 1
    except (OSError, ValueError, KeyError, subprocess.CalledProcessError, tarfile.TarError, packaging.PackageError, reproducibility.ReproError) as error:
        print(json.dumps({"contract": CONTRACT, "result": "FAIL", "error": str(error)}))
        return 1


if __name__ == "__main__":
    sys.exit(main())
