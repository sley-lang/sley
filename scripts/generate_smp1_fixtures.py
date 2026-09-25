#!/usr/bin/env python3
"""Refresh the deterministic S20-410 SMP1 frame, hello, and handshake fixture."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "conformance/smp1/v1"
EXPECTED_FRAMES = ["request", "response", "failure"]
EXPECTED_REJECTIONS = ["length-above-ceiling", "digest-trailer-bit", "truncated-envelope", "magic-bit", "hello-nonzero-bounds"]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="fail when committed fixtures differ from the codec-owned vectors",
    )
    arguments = parser.parse_args()
    completed = subprocess.run(
        [
            "cargo",
            "test",
            "-p",
            "sley-protocol",
            "emit_smp1_vectors_for_fixture_refresh",
            "--",
            "--ignored",
            "--nocapture",
        ],
        cwd=ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        check=True,
    )
    epoch = None
    hellos: dict[str, dict[str, object]] = {}
    selected: dict[str, object] | None = None
    frames: list[dict[str, object]] = []
    rejections: list[dict[str, object]] = []
    for line in completed.stdout.splitlines():
        parts = line.split("|")
        if line.startswith("SMP1_EPOCH|"):
            epoch = parts[1]
        elif line.startswith("SMP1_HELLO|"):
            hellos[parts[1]] = {"frame_hex": parts[2], "frame_id": parts[3], "frame_bytes": len(parts[2]) // 2}
        elif line.startswith("SMP1_SELECTED|"):
            selected = {
                "features": int(parts[5]),
                "handshake_id": parts[4],
                "methods": [int(tag) for tag in parts[6].split(",")],
                "preimage_hex": parts[3],
                "protocol_version": int(parts[1]),
                "schema_epoch_hex": parts[2],
                "transcript_hex": parts[7],
            }
        elif line.startswith("SMP1_FRAME|"):
            frames.append({"frame_bytes": len(parts[2]) // 2, "frame_hex": parts[2], "frame_id": parts[3], "id": parts[1]})
        elif line.startswith("SMP1_REJECT|"):
            rejections.append(
                {
                    "expected_code": parts[3],
                    "expected_numeric": int(parts[4]),
                    "id": parts[1],
                    "input_hex": parts[2],
                }
            )
    if epoch is None or selected is None or set(hellos) != {"client", "server"}:
        raise RuntimeError("incomplete SMP1 emitter output")
    if [frame["id"] for frame in frames] != EXPECTED_FRAMES:
        raise RuntimeError(f"unexpected frame set {[f['id'] for f in frames]}")
    if [rejection["id"] for rejection in rejections] != EXPECTED_REJECTIONS:
        raise RuntimeError(f"unexpected rejection set {[r['id'] for r in rejections]}")
    accepted = {
        "claim": "s20-410-smp1-v1-conformance",
        "contract": "sley2-smp1-v1",
        "frame_contract_tag": 400,
        "frame_domain": "sley2.protocol-frame.v1",
        "frames": frames,
        "generator": "scripts/generate_smp1_fixtures.py",
        "handshake_domain": "sley2.protocol-handshake.v1",
        "hellos": hellos,
        "protocol_epoch_hex": epoch,
        "selected": selected,
    }
    rejected = {
        "claim": "s20-410-smp1-v1-conformance",
        "contract": "sley2-smp1-v1",
        "mutations": rejections,
    }
    rendered = {
        FIXTURES / "accepted.json": json.dumps(accepted, indent=2, sort_keys=True) + "\n",
        FIXTURES / "rejected.json": json.dumps(rejected, indent=2, sort_keys=True) + "\n",
    }
    rendered[FIXTURES / "SHA256SUMS"] = "".join(
        f"{hashlib.sha256(payload.encode()).hexdigest()}  {path.name}\n"
        for path, payload in rendered.items()
    )
    if arguments.check:
        drift = [
            str(path.relative_to(ROOT))
            for path, expected in rendered.items()
            if not path.is_file() or path.read_text(encoding="utf-8") != expected
        ]
        print(
            json.dumps(
                {"drift": drift, "frames": len(frames), "mode": "check", "rejections": len(rejections), "result": "FAIL" if drift else "PASS"},
                sort_keys=True,
            )
        )
        return 1 if drift else 0
    FIXTURES.mkdir(parents=True, exist_ok=True)
    for path, payload in rendered.items():
        path.write_text(payload, encoding="utf-8")
    print(json.dumps({"frames": len(frames), "mode": "write", "rejections": len(rejections), "result": "PASS"}, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
